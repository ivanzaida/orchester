//! The project's Orchestrator chat (`orchester_core::orchestrator`): the runner answers a person's message the desktop
//! kept with one turn of the agent CLI, read-only in the project folder, on the connection and model chosen for the
//! Orchestrator, with the domain tools the desktop serves for the chat. A conversation answers one message at a
//! time; stopping a turn ends its CLI, and what it did until then stays. A turn still thinking that the runner does
//! not answer, when a desktop attaches or the person stops it, was cut short (by the runner before it, or a request
//! whose answer was lost), and is recorded as interrupted.

mod claude;
mod turn;

use std::{collections::HashMap, sync::Arc, time::Duration};

use orchester_core::{
  id::{ChatId, ChatMessageId},
  orchestrator::TIME_LIMIT,
};
use orchester_ipc::{ChatTools, Event, Rejection};
use orchester_store::{AttachmentOwner, ProjectDb};
use parking_lot::Mutex;
use tokio::{sync::oneshot, task::JoinHandle};

use self::turn::Turn;
use super::{Scheduler, failed, idle::check_daily_budget};

/// The turns under way, by conversation.
#[derive(Default)]
pub(super) struct Chats {
  running: Mutex<HashMap<ChatId, Running>>,
  /// How long one answer may run, in milliseconds, when the runner's tests set it shorter; 0: [`TIME_LIMIT`].
  #[cfg(feature = "test-agent")]
  limit_ms: std::sync::atomic::AtomicU64,
}

struct Running {
  message: ChatMessageId,
  /// Ends the turn's CLI; gone once it was used.
  stop: Option<oneshot::Sender<()>>,
  task: JoinHandle<()>,
}

impl Chats {
  /// How long one answer may run: [`TIME_LIMIT`], or in the runner's own tests the limit they set.
  #[cfg(feature = "test-agent")]
  fn time_limit(&self) -> Duration {
    match self.limit_ms.load(std::sync::atomic::Ordering::Relaxed) {
      0 => TIME_LIMIT,
      ms => Duration::from_millis(ms),
    }
  }

  /// How long one answer may run.
  #[cfg(not(feature = "test-agent"))]
  #[expect(clippy::unused_self, reason = "the runner's own tests read their limit from it")]
  fn time_limit(&self) -> Duration {
    TIME_LIMIT
  }

  /// Whether no turn is under way.
  pub(super) fn idle(&self) -> bool {
    self.held().is_empty()
  }

  /// The messages whose turns are under way.
  fn held(&self) -> Vec<ChatMessageId> {
    let mut running = self.running.lock();
    running.retain(|_, turn| !turn.task.is_finished());
    running.values().map(|turn| turn.message).collect()
  }
}

impl Scheduler {
  /// Answers the person's message `message` with a turn of the Orchestrator's agent, using the domain tools at
  /// `tools`; nothing when its turn is under way or ended already.
  pub async fn answer_in_chat(&self, message: ChatMessageId, tools: ChatTools) -> Result<(), Rejection> {
    if self.inner.board.is_stopping() {
      return Err(Rejection::ShuttingDown);
    }
    let db = self.open()?;
    let kept = db
      .chat_turn(message)
      .map_err(|error| failed("cannot read the chat message", &error))?
      .ok_or(Rejection::ChatMessageNotFound)?;
    if !kept.turn.is_thinking() {
      return Ok(());
    }
    let settings = db
      .orchestrator_settings()
      .map_err(|error| failed("cannot read the Orchestrator's settings", &error))?
      .ok_or(Rejection::NoOrchestrator)?;
    // The chat's tokens count toward the daily budget: a used one refuses a turn as it refuses a run.
    check_daily_budget(&db)?;
    let attached = db
      .attached_files(AttachmentOwner::ChatMessage(message))
      .map_err(|error| failed("cannot read the chat message's attachments", &error))?;
    let chat = kept.chat.id();
    let mut running = self.inner.chats.running.lock();
    running.retain(|_, turn| !turn.task.is_finished());
    match running.get(&chat) {
      Some(turn) if turn.message == message => return Ok(()),
      Some(_) => return Err(Rejection::ChatBusy),
      None => {}
    }
    let (stop, stopped) = oneshot::channel();
    let turn = Turn {
      root: self.inner.project_root.clone(),
      project: db.project().id(),
      notifier: self.inner.notifier.clone(),
      vault: Arc::clone(&self.inner.vault),
      chat: kept.chat,
      message,
      text: kept.text,
      attached,
      settings,
      tools,
      time_limit: self.inner.chats.time_limit(),
    };
    let task = tokio::spawn(turn.run(stopped));
    running.insert(
      chat,
      Running {
        message,
        stop: Some(stop),
        task,
      },
    );
    Ok(())
  }

  /// Lets one answer run for `limit` only, so the runner's tests reach the time limit without waiting ten minutes.
  #[cfg(feature = "test-agent")]
  pub fn limit_chat_turns(&self, limit: Duration) {
    let ms = u64::try_from(limit.as_millis()).unwrap_or(u64::MAX).max(1);
    self
      .inner
      .chats
      .limit_ms
      .store(ms, std::sync::atomic::Ordering::Relaxed);
  }

  /// Stops the turn under way in `chat`. A turn the conversation still shows thinking that this runner does not
  /// answer is recorded as interrupted: nothing would end it otherwise.
  pub fn stop_chat(&self, chat: ChatId) -> Result<(), Rejection> {
    {
      let mut running = self.inner.chats.running.lock();
      running.retain(|_, turn| !turn.task.is_finished());
      if let Some(turn) = running.get_mut(&chat) {
        if let Some(stop) = turn.stop.take()
          && stop.send(()).is_err()
        {
          tracing::debug!(%chat, "the chat's turn had ended before it was stopped");
        }
        return Ok(());
      }
    }
    let mut db = self.open()?;
    self.interrupt_chat_turns(&mut db, Some(chat))
  }

  /// Records the turns `db` still has thinking, of conversation `only` when it names one, as interrupted, but those
  /// this runner answers: no runner answers them any more.
  pub(super) fn interrupt_chat_turns(&self, db: &mut ProjectDb, only: Option<ChatId>) -> Result<(), Rejection> {
    let held = self.inner.chats.held();
    let chats = db
      .interrupt_chat_turns(&held, only)
      .map_err(|error| failed("cannot end the chat's interrupted turns", &error))?;
    for chat in chats {
      self.inner.notifier.send(Event::ChatChanged { chat });
    }
    Ok(())
  }
}
