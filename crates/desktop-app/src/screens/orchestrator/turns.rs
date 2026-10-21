//! A conversation as turns: each message a person sent, what the Orchestrator answered it with (its replies and
//! the actions it took, in the order they were kept) and how the turn ended; and each wait of an agent of the
//! project, with the turn it started by itself, if it started one. What is kept while a turn runs belongs to that
//! turn, even when a wait kept meanwhile shows under it.

use std::{collections::BTreeMap, sync::Arc};

use orchester_core::{
  attachment::{Attachment, is_names_only},
  id::ChatMessageId,
  orchestrator::{AgentWait, ChatAction, ChatEntry, ChatEvent, EntryKind, TurnState},
  resource::ModelId,
  time::Timestamp,
};

/// A part of an answer.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Part {
  Reply {
    text: Arc<str>,
    model: ModelId,
    at: Timestamp,
  },
  Action {
    action: Box<ChatAction>,
    at: Timestamp,
    /// Its entry in the conversation, which a proposed start is decided by.
    seq: i64,
  },
  Event {
    event: ChatEvent,
    at: Timestamp,
  },
}

impl Part {
  pub(super) const fn at(&self) -> Timestamp {
    match self {
      Self::Reply { at, .. } | Self::Action { at, .. } | Self::Event { at, .. } => *at,
    }
  }
}

/// One message and its answer, or one wait of an agent and the answer of the turn it started, if any.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Turn {
  /// `None` while it is being sent and not kept yet, and for a wait that started no turn.
  pub(super) message: Option<ChatMessageId>,
  /// The person's words; empty for a wait, and for a message of attachments only.
  pub(super) text: Arc<str>,
  /// The files the person attached to the message.
  pub(super) attachments: Arc<[Attachment]>,
  /// The wait that opens the row instead of a person's message.
  pub(super) wait: Option<Box<AgentWait>>,
  /// Its entry in the conversation; `None` while it is being sent.
  seq: Option<i64>,
  /// When it was kept; `None` while it is being sent.
  pub(super) at: Option<Timestamp>,
  /// When its turn ended, when the runner said.
  pub(super) ended: Option<Timestamp>,
  /// Where its turn stands; `None` for a wait that started no turn.
  pub(super) state: Option<TurnState>,
  pub(super) answer: Vec<Part>,
}

impl Turn {
  /// A message being sent, which the conversation does not hold yet.
  pub(super) fn sending(text: &str) -> Self {
    Self {
      message: None,
      text: Arc::from(text),
      attachments: Arc::from([]),
      wait: None,
      seq: None,
      at: None,
      ended: None,
      state: Some(TurnState::Thinking),
      answer: Vec::new(),
    }
  }

  /// The model that wrote the answer's first reply.
  pub(super) fn model(&self) -> Option<&ModelId> {
    self.answer.iter().find_map(|part| match part {
      Part::Reply { model, .. } => Some(model),
      Part::Action { .. } | Part::Event { .. } => None,
    })
  }

  /// Whether the answer did something through a tool.
  pub(super) fn acted(&self) -> bool {
    self.answer.iter().any(|part| {
      matches!(
        part,
        Part::Action { action, .. }
          if matches!(**action, ChatAction { outcome: orchester_core::orchestrator::ActionOutcome::Done, .. })
      )
    })
  }

  /// The row's key: its message, its wait's entry, or *sending* until it is kept.
  pub(super) fn key(&self) -> String {
    match (self.message, self.seq) {
      (Some(message), _) => message.to_string(),
      (None, Some(seq)) => format!("wait-{seq}"),
      (None, None) => "sending".to_owned(),
    }
  }
}

/// The turns of `entries`, with the files attached to the person's messages (`attached`), oldest first. What comes
/// before the first message or wait belongs to no turn and is left out; a reply, action or event belongs to the newest
/// row with a turn.
pub(super) fn turns(entries: &[ChatEntry], attached: &BTreeMap<ChatMessageId, Vec<Attachment>>) -> Vec<Turn> {
  let mut turns: Vec<Turn> = Vec::new();
  for entry in entries {
    let part = match &entry.kind {
      EntryKind::Person {
        message,
        text,
        turn,
        ended,
      } => {
        let attachments: Arc<[Attachment]> = attached.get(message).map_or_else(|| Arc::from([]), |files| files.as_slice().into());
        // A message of attachments only was kept with their names: it shows by its files alone.
        let words = if is_names_only(text.as_str(), &attachments) { "" } else { text.as_str() };
        turns.push(Turn {
          message: Some(*message),
          text: Arc::from(words),
          attachments,
          wait: None,
          seq: Some(entry.seq),
          at: Some(entry.at),
          ended: *ended,
          state: Some(turn.clone()),
          answer: Vec::new(),
        });
        continue;
      }
      EntryKind::Wait { wait, turn } => {
        turns.push(Turn {
          message: turn.as_ref().map(|turn| turn.message),
          text: Arc::from(""),
          attachments: Arc::from([]),
          wait: Some(Box::new(wait.clone())),
          seq: Some(entry.seq),
          at: Some(entry.at),
          ended: turn.as_ref().and_then(|turn| turn.ended),
          state: turn.as_ref().map(|turn| turn.state.clone()),
          answer: Vec::new(),
        });
        continue;
      }
      EntryKind::Reply { text, model } => Part::Reply {
        text: Arc::from(text.as_str()),
        model: model.clone(),
        at: entry.at,
      },
      EntryKind::Action(action) => Part::Action {
        action: Box::new(action.clone()),
        at: entry.at,
        seq: entry.seq,
      },
      EntryKind::Event(event) => Part::Event {
        event: event.clone(),
        at: entry.at,
      },
    };
    if let Some(turn) = turns.iter_mut().rev().find(|turn| turn.state.is_some()) {
      turn.answer.push(part);
    }
  }
  turns
}
