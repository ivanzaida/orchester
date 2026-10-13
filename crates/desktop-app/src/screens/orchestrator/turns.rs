//! A conversation as turns: each message a person sent, what the Orchestrator answered it with (its replies and
//! the actions it took, in the order they were kept) and how the turn ended.

use std::sync::Arc;

use orchester_core::{
  id::ChatMessageId,
  orchestrator::{ChatAction, ChatEntry, EntryKind, TurnState},
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
    action: ChatAction,
    at: Timestamp,
  },
}

impl Part {
  pub(super) const fn at(&self) -> Timestamp {
    match self {
      Self::Reply { at, .. } | Self::Action { at, .. } => *at,
    }
  }
}

/// One message and its answer.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Turn {
  /// `None` while it is being sent and not kept yet.
  pub(super) message: Option<ChatMessageId>,
  pub(super) text: Arc<str>,
  /// When it was kept; `None` while it is being sent.
  pub(super) at: Option<Timestamp>,
  pub(super) state: TurnState,
  pub(super) answer: Vec<Part>,
}

impl Turn {
  /// A message being sent, which the conversation does not hold yet.
  pub(super) fn sending(text: &str) -> Self {
    Self {
      message: None,
      text: Arc::from(text),
      at: None,
      state: TurnState::Thinking,
      answer: Vec::new(),
    }
  }

  /// The model that wrote the answer's first reply.
  pub(super) fn model(&self) -> Option<&ModelId> {
    self.answer.iter().find_map(|part| match part {
      Part::Reply { model, .. } => Some(model),
      Part::Action { .. } => None,
    })
  }

  /// Whether the answer did something through a tool.
  pub(super) fn acted(&self) -> bool {
    self.answer.iter().any(|part| {
      matches!(
        part,
        Part::Action {
          action: ChatAction {
            outcome: orchester_core::orchestrator::ActionOutcome::Done,
            ..
          },
          ..
        }
      )
    })
  }

  /// The row's key: its message, or *sending* until it is kept.
  pub(super) fn key(&self) -> String {
    self
      .message
      .map_or_else(|| "sending".to_owned(), |message| message.to_string())
  }
}

/// The turns of `entries`, oldest first. What comes before the first message belongs to no turn and is left out.
pub(super) fn turns(entries: &[ChatEntry]) -> Vec<Turn> {
  let mut turns: Vec<Turn> = Vec::new();
  for entry in entries {
    match &entry.kind {
      EntryKind::Person { message, text, turn } => turns.push(Turn {
        message: Some(*message),
        text: Arc::from(text.as_str()),
        at: Some(entry.at),
        state: turn.clone(),
        answer: Vec::new(),
      }),
      EntryKind::Reply { text, model } => {
        if let Some(turn) = turns.last_mut() {
          turn.answer.push(Part::Reply {
            text: Arc::from(text.as_str()),
            model: model.clone(),
            at: entry.at,
          });
        }
      }
      EntryKind::Action(action) => {
        if let Some(turn) = turns.last_mut() {
          turn.answer.push(Part::Action {
            action: action.clone(),
            at: entry.at,
          });
        }
      }
    }
  }
  turns
}
