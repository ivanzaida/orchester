//! Tokens an agent used before it first changed a file (`docs/design/orchester-workspace.md`, "Code
//! index", run details): the usage of the turns that began before its first edit, and when and
//! where that edit was, or that it changed no file. An edit is a file change the CLI reports
//! (Codex `file_change` items, Claude Code's edit tools); a file a shell command writes is not
//! seen. A turn's usage arrives when it completes, so the turn in which the first edit happens
//! counts as before it (Codex reports a whole `exec` turn at its end; Claude Code each message).

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
/// `None` while it changed none.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditProgress {
  pub before: TokenUsage,
  pub first_edit: Option<FirstEdit>,
}

/// Follows one invocation's events to its [`EditProgress`].
#[derive(Clone, Debug, Default)]
pub struct FirstEditTracker {
  progress: EditProgress,
  /// A turn started and has not completed.
  turn_open: bool,
  /// The first edit happened in the turn still open: its usage counts as before the edit.
  edit_turn_open: bool,
}

impl FirstEditTracker {
  /// Takes in `event`, reported at `at`; whether the progress changed.
  pub fn observe(&mut self, event: &AgentEvent, at: Timestamp) -> bool {
    match event {
      AgentEvent::TurnStarted => {
        self.turn_open = true;
        false
      }
      AgentEvent::TurnCompleted { usage } => {
        let counts = self.progress.first_edit.is_none() || self.edit_turn_open;
        self.turn_open = false;
        self.edit_turn_open = false;
        if counts && *usage != TokenUsage::default() {
          self.progress.before += *usage;
          return true;
        }
        false
      }
      AgentEvent::FileChanged { changes, .. } if self.progress.first_edit.is_none() => {
        let Some(change) = changes.first() else {
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
