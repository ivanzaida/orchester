//! The message being sent, shown in the conversation until the conversation holds it (ORC-711): its row is keyed by
//! the id the message is kept under, so the row keeps its element when the message is kept; a new conversation's first
//! message also goes on showing while the conversation it started is read for the first time, so the list never gives
//! way to the conversation's bones between the message sent and the message kept.

use orchester_core::{
  attachment::{NewAttachment, names_text},
  id::{ChatId, ChatMessageId},
  orchestrator::{Queued, TurnState},
};
use orchester_workspace::orchestrator::{chat::ChatView, queue::WhileAnswering, send::Outgoing};

use super::{
  View,
  data::{Reads, Shown},
  turns::{Turn, turns},
};

/// A message being sent: the conversation it goes to, the id it is kept under, and its words, or its files' names
/// when it has none.
pub(super) struct Pending {
  pub(super) chat: ChatId,
  message: ChatMessageId,
  text: String,
}

impl Pending {
  pub(super) fn of(outgoing: Outgoing) -> Self {
    let text = if outgoing.text.trim().is_empty() {
      names_text(outgoing.attachments.iter().map(NewAttachment::name))
    } else {
      outgoing.text
    };
    Self {
      chat: outgoing.chat,
      message: outgoing.message,
      text,
    }
  }
}

impl View {
  /// The first message of the conversation this screen started, while `reads` shows that conversation being read.
  pub(super) fn started_read(&self, reads: &Reads) -> Option<Outgoing> {
    let reading = matches!(reads.shown, Shown::Loading);
    self
      .started
      .get()
      .filter(|started| reading && reads.target == Some(started.chat))
  }

  /// The first message of the conversation this screen started is done with once that conversation is read, or
  /// another one shows; a new conversation, which the route may still name for a pass after the first message was
  /// kept, keeps it.
  pub(super) fn hold_started(&self, reads: &Reads) {
    let Some(started) = self.started.get_untracked() else {
      return;
    };
    let done = match reads.shown {
      Shown::New => false,
      Shown::Loading => reads.target.is_some_and(|target| target != started.chat),
      Shown::Chat(_) | Shown::Failed(_) | Shown::Missing => true,
    };
    if done {
      self.started.set(None);
    }
  }
}

/// The turns of `shown` (none for a new conversation), and the message being sent (`pending`) until the conversation
/// holds it: as a turn of its own, or as it will wait while the Orchestrator answers.
pub(super) fn all_turns(view: &View, shown: Option<&ChatView>, pending: Option<&Pending>) -> Vec<Turn> {
  let mut all = shown.map(turns).unwrap_or_default();
  if let Some(pending) = pending
    && !all.iter().any(|turn| turn.message == Some(pending.message))
  {
    let waits = view.how.get().map(|how| match how {
      WhileAnswering::AfterAnswer => TurnState::Queued(Queued::AfterAnswer),
      WhileAnswering::Now => TurnState::Steering,
      WhileAnswering::StopAndSend => TurnState::Queued(Queued::Next),
    });
    all.push(Turn::sending(
      pending.message,
      &pending.text,
      waits.unwrap_or(TurnState::Thinking),
    ));
  }
  all
}
