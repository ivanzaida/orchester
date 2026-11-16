//! What the Orchestrator decided while nobody asked (`docs/plan/orchestrator-autonomy/OA-22-…`). In a turn Orchester
//! started by itself, a call that is a judgement (accepting a step, a return, a retry, a raise, an answer, a decision
//! on a plan, a request allowed or denied) carries a one-sentence [`Reason`], and what it did is kept as a
//! [`Decision`] of the project beside its action, for the person to read afterwards: open until they mark it seen or
//! overrule it ([`Review`]). A turn a person's message started makes none: they asked.

use crate::{
  id::{ChatId, RunId},
  time::Timestamp,
};

/// The longest reason kept, in characters: a sentence, not a report.
pub const MAX_REASON: usize = 400;

/// Why the chat decided as it did, in its own words on one line. It is the agent's text: shown as text, never as
/// something that acts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reason(String);

impl Reason {
  /// `text` as a reason, its line breaks and runs of blanks read as one blank; `None` when nothing is left, or it is
  /// longer than [`MAX_REASON`] characters.
  pub fn new(text: &str) -> Option<Self> {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!line.is_empty() && line.chars().count() <= MAX_REASON).then_some(Self(line))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// What undoes a decision, where something does; nothing is promised for the others.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Undo {
  /// A step was accepted as it was: the run can be returned to that step.
  ReturnToStep,
  /// A task was parked for the person: it can be given back to the chat.
  GiveBack,
}

impl Undo {
  /// The name it is stored and told by.
  pub const fn name(self) -> &'static str {
    match self {
      Self::ReturnToStep => "return_to_step",
      Self::GiveBack => "give_back",
    }
  }

  /// The undo stored as `name`; `None` for a name this version does not know.
  pub fn named(name: &str) -> Option<Self> {
    [Self::ReturnToStep, Self::GiveBack]
      .into_iter()
      .find(|undo| undo.name() == name)
  }
}

/// Whether the person looked at a decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Review {
  Open,
  /// The person marked it seen.
  Seen {
    at: Timestamp,
  },
  /// The person overruled it, in a message of theirs in its conversation.
  Overruled {
    at: Timestamp,
  },
}

impl Review {
  pub const fn is_open(self) -> bool {
    matches!(self, Self::Open)
  }

  /// The name its state is stored and told by.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Open => "open",
      Self::Seen { .. } => "seen",
      Self::Overruled { .. } => "overruled",
    }
  }

  /// Whether a decision reviewed as `self` may become `next`: an open one is seen or overruled, and one that was seen
  /// may still be overruled. Nothing opens again, and an overruling is not taken back.
  pub const fn may_become(self, next: Self) -> bool {
    matches!(
      (self, next),
      (Self::Open, Self::Seen { .. } | Self::Overruled { .. }) | (Self::Seen { .. }, Self::Overruled { .. })
    )
  }
}

/// A decision as it is made: by the turn of conversation `chat`, through tool `tool`, on `task` (its key as people
/// read it) and `run` when the call named them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewDecision {
  pub chat: ChatId,
  pub tool: String,
  pub task: Option<String>,
  pub run: Option<RunId>,
  pub reason: Reason,
  pub undo: Option<Undo>,
  pub at: Timestamp,
}

/// A decision the project keeps, by its number in the project, with whether the person looked at it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
  pub number: i64,
  pub made: NewDecision,
  pub review: Review,
}
