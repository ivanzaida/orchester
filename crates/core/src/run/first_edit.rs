//! Tokens an agent used before it first changed a file (`docs/design/orchester-workspace.md`, "Code
//! index", run details): the usage of the turns that began before its first edit, and when and
//! where that edit was, or that it changed no file. An edit is a change to a file in the repository
//! (the run's worktree) that the agent reports: Codex `file_change` items, Claude Code's edit tools,
//! Orchester's `edit_file` and `create_file` for either CLI, a native agent's file tools. A change in
//! a writable folder outside the worktree is no edit of the repository, and a file a shell command
//! writes is not seen. A turn's usage arrives when it completes, so the turn in which the first edit happens
//! counts as before it: Codex reports a whole turn's usage at its end, which then makes the count an
//! upper bound ([`EditProgress::at_most`]); Claude Code reports each message, and a message that
//! makes the edit completes before its result does. Claude Code's per-message usage can report less
//! output than its final `result` does; what the result adds after the edit is not counted. The same events count
//! the agent's searches of the code index it got, over the whole invocation (`super::index_search`), which Usage
//! shows for every invocation, one that changes no file too.

use serde::{Deserialize, Serialize};

use super::TokenUsage;
use crate::{connection::stream::AgentEvent, time::Timestamp};

/// The first file an agent changed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstEdit {
  pub at: Timestamp,
  /// As the CLI reported it.
  pub path: String,
}

/// What an invocation used before its agent first changed a file, and that edit; `first_edit` is
/// `None` while it changed none. Also how often it searched the code index, over the whole invocation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditProgress {
  pub before: TokenUsage,
  pub first_edit: Option<FirstEdit>,
  /// `before` holds a whole turn that went on after the first edit: the tokens before it were at
  /// most that many.
  #[serde(default)]
  pub at_most: bool,
  /// The searches and reads of the code index the agent got ([`super::searches_code_index`]), before and after its
  /// first edit; `None` for an invocation recorded before they were counted.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub index_searches: Option<u64>,
}

impl EditProgress {
  /// The progress of an invocation that has just started: nothing used, no edit, no index search.
  pub fn started() -> Self {
    Self {
      index_searches: Some(0),
      ..Self::default()
    }
  }

  /// What a step execution used before its first edit, from the progress of its invocations in
  /// launch order: the invocations up to the one that made the first edit. Its index searches are
  /// those of all its invocations, unknown when one's are.
  pub fn of_execution<'a>(parts: impl IntoIterator<Item = &'a Self>) -> Self {
    let parts: Vec<&Self> = parts.into_iter().collect();
    let mut total = Self {
      index_searches: parts.iter().map(|part| part.index_searches).sum(),
      ..Self::default()
    };
    for part in parts {
      total.before += part.before;
      if part.first_edit.is_some() {
        total.first_edit.clone_from(&part.first_edit);
        total.at_most = part.at_most;
        break;
      }
    }
    total
  }
}

/// Follows one invocation's events to its [`EditProgress`], from [`EditProgress::started`].
#[derive(Clone, Debug)]
pub struct FirstEditTracker {
  progress: EditProgress,
  /// The agent got the code index: its searches of it are counted.
  index_given: bool,
  /// A turn started and has not completed.
  turn_open: bool,
  /// The first edit happened in the turn still open: its usage counts as before the edit.
  edit_turn_open: bool,
}

impl Default for FirstEditTracker {
  fn default() -> Self {
    Self::new(false)
  }
}

impl FirstEditTracker {
  /// Follows an invocation whose agent got the code index when `index_given`; without it no event is a search of it.
  pub fn new(index_given: bool) -> Self {
    Self {
      progress: EditProgress::started(),
      index_given,
      turn_open: false,
      edit_turn_open: false,
    }
  }

  /// Takes in `event`, reported at `at`; whether the progress changed. `in_repository` tells whether a
  /// changed path, as the agent reported it, lies in the repository.
  pub fn observe(&mut self, event: &AgentEvent, at: Timestamp, in_repository: impl Fn(&str) -> bool) -> bool {
    if self.index_given && super::searches_code_index(event) {
      *self.progress.index_searches.get_or_insert(0) += 1;
      return true;
    }
    match event {
      AgentEvent::TurnStarted => {
        self.turn_open = true;
        false
      }
      AgentEvent::TurnCompleted { usage } => {
        let counts = self.progress.first_edit.is_none() || self.edit_turn_open;
        let after_edit = self.edit_turn_open;
        self.turn_open = false;
        self.edit_turn_open = false;
        if counts && *usage != TokenUsage::default() {
          self.progress.before += *usage;
          self.progress.at_most |= after_edit;
          return true;
        }
        false
      }
      AgentEvent::FileChanged { changes, .. } if self.progress.first_edit.is_none() => {
        let Some(change) = changes.iter().find(|change| in_repository(&change.path)) else {
          return false;
        };
        self.progress.first_edit = Some(FirstEdit {
          at,
          path: change.path.clone(),
        });
        self.edit_turn_open = self.turn_open;
        true
      }
      _ => false,
    }
  }

  pub const fn progress(&self) -> &EditProgress {
    &self.progress
  }
}
