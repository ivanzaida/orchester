//! Messages a person sends now (*Send now*, ORC-92) into the answer under way, as a step's agent gets them
//! (`super::super::step::worker::steer`): Claude Code gets each as another stream-json user message and prints it back
//! as it takes it; Codex gets it with `turn/steer` and shows it took it as a user message. A message the turn takes
//! joins it ([`TurnState::Joined`]): that turn answers it. One the CLI refuses, that cannot be written in
//! [`WRITE_TIME`], or that comes once the turn takes no more input, is left for a turn of its own after the answer
//! ([`Queued::NotTaken`]); so is every one the turn still holds when it ends ([`Turn::release_steering`]), one Codex
//! accepted but recorded only as its turn ended included: an answer of its own is safer than a message the
//! conversation shows answered that no answer read, even if Codex then reads it twice.

use std::time::Duration;

use orchester_core::{
  attachment::input::{AttachedFile, claude_content, codex_items, taken_text, user_line},
  id::ChatMessageId,
  orchestrator::{Queued, TurnState},
};
use orchester_store::{AttachmentOwner, ProjectDb};

use super::turn::Turn;
use crate::{
  execution::step::app_server::{AppServerSession, STEER_REQUEST, SteerAnswer},
  supervisor::Supervised,
};

/// How long a message may take to be written to the CLI's input: a CLI that stops reading it would otherwise hold the
/// turn, which must go on reading the CLI's output meanwhile.
const WRITE_TIME: Duration = Duration::from_secs(5);
/// What follows [`STEER_REQUEST`] in the id of a chat's `turn/steer`; the rest names the message.
const CHAT_STEER: &str = "chat-";

/// The messages handed to the turn's CLI that it has not taken yet.
#[derive(Default)]
pub(super) struct Steering {
  sent: Vec<Sent>,
}

struct Sent {
  message: ChatMessageId,
  /// The text the CLI shows it took the message by: its words, and the lines of the files attached to it.
  taken: String,
}

impl Steering {
  /// The message the CLI shows it took by `text`, no longer waiting.
  fn take_text(&mut self, text: &str) -> Option<ChatMessageId> {
    let at = self.sent.iter().position(|sent| sent.taken.trim() == text.trim())?;
    Some(self.sent.remove(at).message)
  }

  /// Whether `message` still waited to be taken; it no longer does.
  fn take(&mut self, message: ChatMessageId) -> bool {
    let before = self.sent.len();
    self.sent.retain(|sent| sent.message != message);
    self.sent.len() < before
  }
}

impl Turn {
  /// Writes `message` to Claude Code's input as another user message, unless the CLI takes no more input.
  pub(super) async fn steer_claude(&self, process: &Supervised, steering: &mut Steering, message: ChatMessageId) {
    if !process.takes_input() {
      return self.not_taken(message, "the turn ended before the message");
    }
    let content = match self.steered(message) {
      Ok((text, attached)) => match claude_content(&text, &attached) {
        Ok(content) => content,
        Err(error) => return self.not_taken(message, &format!("its files cannot be read: {error}")),
      },
      Err(why) => return self.not_taken(message, &why),
    };
    steering.sent.push(Sent {
      message,
      taken: taken_text(&content),
    });
    let written = tokio::time::timeout(WRITE_TIME, process.send_line(user_line(&content))).await;
    if !matches!(written, Ok(Ok(()))) && steering.take(message) {
      self.not_taken(message, "the CLI did not read its input in time");
    }
  }

  /// Adds `message` to Codex's running turn with `turn/steer`, unless no turn runs.
  pub(super) async fn steer_codex(
    &self,
    (app, process): (&AppServerSession, &Supervised),
    steering: &mut Steering,
    message: ChatMessageId,
  ) {
    if !app.turn_working() {
      return self.not_taken(message, "no Codex turn runs to take the message");
    }
    let input = match self.steered(message) {
      Ok((text, attached)) => match codex_items(&text, &attached) {
        Ok(input) => input,
        Err(error) => return self.not_taken(message, &format!("its files cannot be read: {error}")),
      },
      Err(why) => return self.not_taken(message, &why),
    };
    steering.sent.push(Sent {
      message,
      taken: taken_text(&input),
    });
    let id = format!("{STEER_REQUEST}{CHAT_STEER}{message}");
    let written = tokio::time::timeout(WRITE_TIME, app.steer(process, &id, &input)).await;
    if !matches!(written, Ok(Ok(()))) && steering.take(message) {
      self.not_taken(message, "Codex did not read its input in time");
    }
  }

  /// Codex's answers to the chat's `turn/steer` requests: a refused message gets a turn of its own.
  pub(super) fn steer_answered(&self, steering: &mut Steering, answers: Vec<SteerAnswer>) {
    for answer in answers {
      let Some(message) = answer
        .id
        .strip_prefix(STEER_REQUEST)
        .and_then(|id| id.strip_prefix(CHAT_STEER))
        .and_then(|id| id.parse::<ChatMessageId>().ok())
      else {
        continue;
      };
      if let Some(reason) = answer.refused
        && steering.take(message)
      {
        self.not_taken(message, &format!("Codex refused it: {reason}"));
      }
    }
  }

  /// The CLI took `text` into its conversation: the message sent with it joins the turn.
  pub(super) fn took(&self, steering: &mut Steering, text: &str) {
    if let Some(message) = steering.take_text(text) {
      tracing::info!(chat = %self.chat.id(), %message, "the answer under way took a message sent now");
      self.record(|db| {
        db.change_chat_turn(message, &[TurnState::Steering], &TurnState::Joined)
          .map(drop)
      });
    }
  }

  /// Leaves every message of the conversation still handed to this turn, which ended, for turns of their own.
  pub(super) fn release_steering(&self) {
    self.record(|db| {
      let released = db.release_chat_steering(self.chat.id())?;
      if !released.is_empty() {
        tracing::info!(chat = %self.chat.id(), count = released.len(), "messages the answer did not take wait for it");
      }
      Ok(())
    });
  }

  /// The words and files of `message`, as its CLI gets them.
  fn steered(&self, message: ChatMessageId) -> Result<(String, Vec<AttachedFile>), String> {
    let db = ProjectDb::open(&self.root).map_err(|error| error.to_string())?;
    let kept = db
      .chat_turn(message)
      .map_err(|error| error.to_string())?
      .ok_or_else(|| "the message is no longer kept".to_owned())?;
    let attached = db
      .attached_files(AttachmentOwner::ChatMessage(message))
      .map_err(|error| error.to_string())?;
    Ok((kept.text.as_str().to_owned(), attached))
  }

  /// The turn does not take `message`, for `why`: it gets a turn of its own after the answer.
  fn not_taken(&self, message: ChatMessageId, why: &str) {
    tracing::info!(chat = %self.chat.id(), %message, why, "a message sent now waits for the answer to end");
    self.record(|db| {
      db.change_chat_turn(message, &[TurnState::Steering], &TurnState::Queued(Queued::NotTaken))
        .map(drop)
    });
  }
}
