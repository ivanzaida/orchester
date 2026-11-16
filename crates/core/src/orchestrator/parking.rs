//! A task the Orchestrator parked for the person (`docs/plan/orchestrator-autonomy/OA-15-…`). When the chat cannot
//! move a task (a decision that is the person's, a request it may not allow, a ceiling, a tool that does not answer)
//! it says so in a form the person can act on ([`Parking`]): what kind of thing is needed, why it could not, and what
//! exactly to do. A parked task is started by nobody and its events are tried no more, and the chat goes on with the
//! rest. The parking ends when the person acts on the task or gives it back.

use crate::{
  id::{ChatId, RunId, TaskId},
  time::Timestamp,
};

/// The longest *why* and *what to do* kept, in characters each.
pub const MAX_WORDS: usize = 600;

/// The chat's words of a parking, on one line; they are the agent's text, shown as text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Words(String);

impl Words {
  /// `text` as words of a parking, its line breaks and runs of blanks read as one blank; `None` when nothing is left,
  /// or it is longer than [`MAX_WORDS`] characters.
  pub fn new(text: &str) -> Option<Self> {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!line.is_empty() && line.chars().count() <= MAX_WORDS).then_some(Self(line))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// What kind of thing a parked task needs of the person.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Needs {
  /// A decision that is theirs: irreversible, outward, or one the brief does not settle.
  Decision,
  /// A request of an agent only they may allow.
  Approval,
  /// A sign-in, or credentials.
  SignIn,
  /// A tool that does not answer, which they must bring back.
  Tool,
  /// A ceiling of what leads the task was reached.
  Ceiling,
  Other,
}

impl Needs {
  pub const ALL: [Self; 6] = [
    Self::Decision,
    Self::Approval,
    Self::SignIn,
    Self::Tool,
    Self::Ceiling,
    Self::Other,
  ];

  /// The name it is stored and told by.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Decision => "decision",
      Self::Approval => "approval",
      Self::SignIn => "sign_in",
      Self::Tool => "tool",
      Self::Ceiling => "ceiling",
      Self::Other => "other",
    }
  }

  /// What is stored or told as `name`; `None` for a name this version does not know.
  pub fn named(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|needs| needs.name() == name)
  }
}

/// A task parked for the person: by the turn of conversation `by`, with the run that waited then, when one did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parking {
  pub task: TaskId,
  pub run: Option<RunId>,
  pub needs: Needs,
  /// Why the chat could not go on with it.
  pub why: Words,
  /// What exactly the person is to do.
  pub to_do: Words,
  pub by: ChatId,
  pub at: Timestamp,
}

/// How a parking ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unparked {
  /// The person gave the task back to the chat.
  GivenBack,
  /// The task's run moved since it was parked, or it has another: someone acted on it.
  Moved,
}
