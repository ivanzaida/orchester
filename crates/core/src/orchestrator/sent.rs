//! What the project's Orchestrator chat wrote on the person's side of a task's conversation (ORC-45): a message to a
//! run's agents or the answer to a question, the conversation it came from and how it went out, sent by the chat
//! itself while the person's switch let it, or proposed by it and applied by the person. A proposal is applied only
//! by a person: automation never takes the decision ([`super::change`]).

use crate::id::ChatId;

/// How something the Orchestrator chat wrote went out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SentWay {
  /// The chat sent it itself: *Let the Orchestrator answer agents* was on.
  Direct,
  /// The chat proposed it and the person applied the proposal.
  AppliedByPerson,
}

impl SentWay {
  /// The word the database and the domain tools keep.
  pub const fn word(self) -> &'static str {
    match self {
      Self::Direct => "direct",
      Self::AppliedByPerson => "applied_by_person",
    }
  }

  /// The way `word` names; `None` for a word it never writes.
  pub fn of_word(word: &str) -> Option<Self> {
    [Self::Direct, Self::AppliedByPerson]
      .into_iter()
      .find(|way| way.word() == word)
  }
}

/// A message or an answer the Orchestrator chat wrote: its conversation and how it went out, each `None` for one
/// kept before they were recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FromOrchestrator {
  pub chat: Option<ChatId>,
  pub way: Option<SentWay>,
}

impl FromOrchestrator {
  /// Written in conversation `chat`, gone out `way`.
  pub const fn new(chat: ChatId, way: SentWay) -> Self {
    Self {
      chat: Some(chat),
      way: Some(way),
    }
  }
}
