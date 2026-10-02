//! A person's messages sent now (*Send now*, `docs/design/orchester-workspace.md`, "Conversation
//! (merged with Activity)"), given to the agent while its turn runs. Codex gets each with
//! `turn/steer` and shows it took it as a `userMessage` item; Claude Code gets it as another
//! stream-json user message and prints it back as it takes it. Both take it after the tool call
//! that is running (checked live 2026-10-02, `docs/evidence/orc-61-send-now-2026-10-02.md`). A
//! message is delivered when the CLI shows it took it: mid-turn, or for Claude Code at its next turn
//! when the turn ended first. One the CLI refuses, or that cannot be written to it in [`WRITE_TIME`],
//! is not delivered; if the CLI takes it later after all, it is delivered then.
//! One the turn ended before (Codex had no turn to add it to, or Claude Code's input was closed),
//! and one still on its way when the CLI ends, stays queued and is read when the turn ends. One that
//! comes before Codex's turn has started is given to the turn as it starts.
//!
//! Codex keeps a message it accepted (`turn/steer` answered with the turn) for its turn: it gives it
//! to the model at the next sampling, or, when the turn finishes first, records it into the thread
//! before `turn/completed` (Codex's `on_task_finished`). Such a message is delivered at the turn's
//! end then, so it is never queued for the next turn as well; only a turn interrupted before taking it
//! leaves it queued (`Update::TurnEnded`), as does a process that ends without completing its turn.

use std::{io, time::Duration};

use orchester_core::{
  connection::invocation::input_line,
  conversation::{Arrival, Delivery, Message, Undelivered},
  id::MessageId,
  store::ConversationStore,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::StoreError;

use super::{
  super::app_server::{AppServerSession, STEER_REQUEST, SteerAnswer},
  Transport, Watch, Worker,
};

/// What Codex 0.159.2 answers to `turn/steer` while no turn runs.
const NO_ACTIVE_TURN: &str = "no active turn";
/// How long a message may take to be written to the CLI's input: a CLI that stops reading it would
/// otherwise hold the worker, which must go on reading the CLI's output meanwhile.
const WRITE_TIME: Duration = Duration::from_secs(5);

/// Messages handed to the CLI that it has not taken yet.
#[derive(Default)]
pub(in super::super) struct Steering {
  sent: Vec<Sent>,
  /// Messages for a Codex turn that has not started yet.
  early: Vec<Message>,
}

struct Sent {
  message: Message,
  /// The turns Claude Code had ended when the message was written: one taken after more ended
  /// arrived at a turn's end.
  results_before: usize,
  /// Codex accepted it for its running turn.
  accepted: bool,
}

impl Steering {
  /// The message sent with `id`, no longer waiting.
  fn take(&mut self, id: MessageId) -> Option<Sent> {
    let at = self.sent.iter().position(|sent| sent.message.id() == id)?;
    Some(self.sent.remove(at))
  }

  /// Notes that Codex accepted the message sent with `id` for its turn.
  fn accept(&mut self, id: MessageId) {
    if let Some(sent) = self.sent.iter_mut().find(|sent| sent.message.id() == id) {
      sent.accepted = true;
    }
  }

  /// The messages Codex accepted and has not shown it took, no longer waiting.
  fn take_accepted(&mut self) -> Vec<Sent> {
    let (accepted, waiting) = std::mem::take(&mut self.sent)
      .into_iter()
      .partition(|sent| sent.accepted);
    self.sent = waiting;
    accepted
  }

  /// The message whose text is `text`, no longer waiting.
  fn take_text(&mut self, text: &str) -> Option<Sent> {
    let at = self
      .sent
      .iter()
      .position(|sent| sent.message.body().as_str() == text.trim())?;
    Some(self.sent.remove(at))
  }
}

impl Worker {
  /// Hands `messages` to the CLI for its running turn. One that cannot be written in time is not
  /// delivered; one for a turn that already ended stays queued.
  pub(super) async fn steer(&mut self, watch: &mut Watch, messages: Vec<Message>) -> Result<(), StoreError> {
    for message in messages {
      if self.app_server().is_some_and(AppServerSession::awaits_turn) {
        watch.steering.early.push(message);
        continue;
      }
      let Some((written, results_before)) = self.write(&message).await else {
        tracing::info!(message = %message.id(), "the turn ended before the message; it is read at its end");
        continue;
      };
      let id = message.id();
      // A message that took too long may still reach the CLI: it stays known until it ends.
      watch.steering.sent.push(Sent {
        message,
        results_before,
        accepted: false,
      });
      if let Err(error) = written {
        tracing::warn!(message = %id, %error, "the message could not be written to the agent's CLI");
        self.not_delivered(watch, id, Undelivered::Unwritable)?;
      }
    }
    Ok(())
  }

  /// Gives Codex's turn, once it has started, the messages that came before it.
  pub(super) async fn steer_early(&mut self, watch: &mut Watch) -> Result<(), StoreError> {
    if watch.steering.early.is_empty() || !self.app_server().is_some_and(AppServerSession::turn_working) {
      return Ok(());
    }
    let early = std::mem::take(&mut watch.steering.early);
    self.steer(watch, early).await
  }

  /// Writes `message` to the CLI for its running turn, with the turns Claude Code had ended; `None`
  /// when no turn runs that could take it.
  async fn write(&self, message: &Message) -> Option<(io::Result<()>, usize)> {
    let text = message.body().as_str();
    match &self.transport {
      Transport::AppServer(app) if app.turn_working() => {
        let id = format!("{STEER_REQUEST}{}", message.id());
        let written = tokio::time::timeout(WRITE_TIME, app.steer(&self.process, &id, text)).await;
        Some((in_time(written), 0))
      }
      Transport::Claude(claude) if self.process.takes_input() => {
        let results = claude.results();
        let written = tokio::time::timeout(WRITE_TIME, self.process.send_line(input_line(text))).await;
        Some((in_time(written), results))
      }
      Transport::AppServer(_) | Transport::Claude(_) => None,
    }
  }

  /// Codex's answers to `turn/steer`: an accepted message waits for its turn to take it, at the latest
  /// as the turn ends; one refused because the turn ended stays queued, any other refusal is not
  /// delivered.
  pub(super) fn steer_answered(&self, watch: &mut Watch, answers: Vec<SteerAnswer>) -> Result<(), StoreError> {
    for answer in answers {
      let Some(id) = answer
        .id
        .strip_prefix(STEER_REQUEST)
        .and_then(|id| id.parse::<MessageId>().ok())
      else {
        continue;
      };
      let Some(reason) = answer.refused else {
        watch.steering.accept(id);
        continue;
      };
      let Some(sent) = watch.steering.take(id) else {
        continue;
      };
      let turn_over = reason.contains(NO_ACTIVE_TURN) || self.app_server().is_some_and(|app| !app.turn_working());
      if turn_over {
        tracing::info!(message = %id, "the turn ended before the message; it is read at its end");
        continue;
      }
      tracing::warn!(message = %id, %reason, "Codex did not add the message to its turn");
      self.not_delivered(watch, sent.message.id(), Undelivered::Refused)?;
    }
    Ok(())
  }

  /// The CLI took `text` into its conversation: the message sent with it is delivered, mid-turn, or
  /// at a turn's end when Claude Code ended a turn since it was written (`results`).
  pub(super) fn input_taken(&self, watch: &mut Watch, text: &str, results: Option<usize>) -> Result<(), StoreError> {
    let Some(sent) = watch.steering.take_text(text) else {
      return Ok(());
    };
    let arrival = match results {
      Some(results) if results > sent.results_before => Arrival::TurnEnd,
      _ => Arrival::MidTurn,
    };
    let (run, execution, invocation) = (watch.invocation.run, watch.invocation.execution, watch.invocation.id);
    self.record(watch, sent.message.id(), |message| {
      message.deliver(run, (execution, invocation), Timestamp::now(), arrival)
    })
  }

  /// Codex's turn ended and kept the messages it accepted but did not show it took: they are
  /// delivered at the turn's end, and not queued for the next turn as well.
  pub(super) fn kept_at_turn_end(&self, watch: &mut Watch) -> Result<(), StoreError> {
    let (run, execution, invocation) = (watch.invocation.run, watch.invocation.execution, watch.invocation.id);
    for sent in watch.steering.take_accepted() {
      self.record(watch, sent.message.id(), |message| {
        message.deliver(run, (execution, invocation), Timestamp::now(), Arrival::TurnEnd)
      })?;
    }
    Ok(())
  }

  fn not_delivered(&self, watch: &mut Watch, id: MessageId, reason: Undelivered) -> Result<(), StoreError> {
    let (run, execution) = (watch.invocation.run, watch.invocation.execution);
    self.record(watch, id, |message| {
      message.not_delivered(run, execution, Timestamp::now(), reason)
    })
  }

  /// Changes message `id` as stored, while it is not delivered, and announces the run's change.
  fn record(
    &self,
    watch: &mut Watch,
    id: MessageId,
    change: impl FnOnce(&mut Message) -> Result<(), orchester_core::conversation::MessageError>,
  ) -> Result<(), StoreError> {
    let stored = watch
      .db
      .messages(self.task)?
      .into_iter()
      .find(|message| message.id() == id);
    let pending = |message: &Message| {
      matches!(
        message.delivery(),
        Some(Delivery::Queued | Delivery::NotDelivered { .. })
      )
    };
    let Some(mut message) = stored.filter(pending) else {
      tracing::debug!(message = %id, "the message was delivered already");
      return Ok(());
    };
    if let Err(error) = change(&mut message) {
      tracing::warn!(message = %id, %error, "the message's delivery does not change");
      return Ok(());
    }
    watch.db.save_deliveries(&[message])?;
    self.notifier.send(Event::RunChanged {
      run: watch.invocation.run,
      task: self.task,
    });
    Ok(())
  }
}

/// A write that did not end in [`WRITE_TIME`] failed.
fn in_time(written: Result<io::Result<()>, tokio::time::error::Elapsed>) -> io::Result<()> {
  written.unwrap_or_else(|_| {
    Err(io::Error::new(
      io::ErrorKind::TimedOut,
      "the CLI did not read its input in time",
    ))
  })
}
