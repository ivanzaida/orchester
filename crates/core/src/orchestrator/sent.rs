//! What the project's Orchestrator chat wrote on the person's side of a task's conversation (ORC-45): a message to a
//! run's agents or the answer to a question, the conversation it came from and how it went out, sent by the chat
//! itself while the person's switch let it, or proposed by it and applied by the person. How it went out is who
//! decided it ([`Answerer`]): automation when the chat sent it, the person when they applied its proposal.

use crate::{id::ChatId, run::Answerer};

/// How something the Orchestrator chat wrote went out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SentWay {
  /// The chat sent it itself: *Let the Orchestrator answer agents* was on.
  Direct,
  /// The chat proposed it and the person applied the proposal.
  AppliedByPerson,
}

impl SentWay {
  /// The way of what `by` decided: the person applied a proposal, automation sent it.
  pub const fn of(by: Answerer) -> Self {
    match by {
      Answerer::Automation => Self::Direct,
      Answerer::Person => Self::AppliedByPerson,
    }
  }

  /// The word the domain tools report.
  pub const fn word(self) -> &'static str {
    match self {
      Self::Direct => "direct",
      Self::AppliedByPerson => "applied_by_person",
    }
  }
}

/// A message or an answer the Orchestrator chat wrote: its conversation and how it went out, both `None` for one
/// kept before its conversation was recorded.
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

  /// Written in conversation `chat` (`None` before it was kept) and decided by `by`.
  pub const fn of(chat: Option<ChatId>, by: Answerer) -> Self {
    match chat {
      Some(chat) => Self::new(chat, SentWay::of(by)),
      None => Self { chat: None, way: None },
    }
  }
}
