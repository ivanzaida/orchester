//! The project's Orchestrator chat (`orchester_core::orchestrator`): the runner answers a person's message the desktop
//! kept with one turn of the agent CLI, read-only in the project folder, on the connection and model chosen for the
//! Orchestrator, with the domain tools the desktop serves for the chat. A conversation answers one message at a
//! time; stopping a turn ends its CLI, and what it did until then stays. A turn still thinking that the runner does
//! not answer, when a desktop attaches or the person stops it, was cut short (by the runner before it, or a request
//! whose answer was lost), and is recorded as interrupted.

mod turn;

use std::collections::HashMap;

use orchester_core::id::{ChatId, ChatMessageId};
use orchester_ipc::{ChatTools, Event, Rejection};
use orchester_store::ProjectDb;
use parking_lot::Mutex;
use tokio::{sync::oneshot, task::JoinHandle};

use self::turn::Turn;
use super::{Scheduler, failed};

/// The turns under way, by conversation.
#[derive(Default)]
pub(super) struct Chats {
  running: Mutex<HashMap<ChatId, Running>>,
}

struct Running {
  message: ChatMessageId,
  /// Ends the turn's CLI; gone once it was used.
  stop: Option<oneshot::Sender<()>>,
  task: JoinHandle<()>,
}

impl Chats {
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
      chat: kept.chat,
      message,
      text: kept.text,
      settings,
      tools,
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
