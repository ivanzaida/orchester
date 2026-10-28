//! A project's workspace settings (Project settings › Runs): the warm folders, the optional warm-up command and the
//! free space the disk guard keeps. How many workspaces there are is the project's agents at once
//! ([`crate::limits::AgentSlots`]), not a setting of its own.

use thiserror::Error;

use super::{
  WorkspaceNumber,
  disk::KeepFree,
  warm::{WarmChoice, WarmFolder, WarmPattern},
};
use crate::time::Timestamp;

/// The last warm-up that failed, until one succeeds: the hint line of Project settings › Runs › Warm-up shows it with
/// its log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarmUpFailure {
  pub workspace: WorkspaceNumber,
  /// `None` when the command could not start or was ended.
  pub exit_code: Option<i32>,
  pub failed_at: Timestamp,
  /// Its output, a file in the project's storage, relative to the repository.
  pub log: String,
}

/// The longest warm-up command, in bytes.
const MAX_COMMAND_BYTES: usize = 2000;

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum WarmUpCommandError {
  #[error("the warm-up command is empty")]
  Empty,
  #[error("the warm-up command is one line")]
  MultipleLines,
  #[error("the warm-up command is at most {MAX_COMMAND_BYTES} bytes long")]
  TooLong,
}

/// The command that builds idle workspaces on the base branch while nothing works in the project, such as
/// `cargo build --tests`, run by the project's shell at low priority.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WarmUpCommand(String);

impl WarmUpCommand {
  pub fn new(text: &str) -> Result<Self, WarmUpCommandError> {
    let text = text.trim();
    if text.is_empty() {
      return Err(WarmUpCommandError::Empty);
    }
    if text.contains(['\n', '\r']) {
      return Err(WarmUpCommandError::MultipleLines);
    }
    if text.len() > MAX_COMMAND_BYTES {
      return Err(WarmUpCommandError::TooLong);
    }
    Ok(Self(text.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// The workspace settings of a project.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceSettings {
  /// Whether the project uses warm workspaces: new runs borrow them, and the disk guard keeps `keep_free` free. Off
  /// until a person turns it on in Project settings › Runs; until then every new run gets a worktree of its own, as
  /// before workspaces, and the disk guard neither refuses nor pauses runs.
  pub enabled: bool,
  pub keep_free: KeepFree,
  pub warm_up: Option<WarmUpCommand>,
  /// In the order the person sees them.
  pub folders: Vec<WarmFolder>,
}

impl WorkspaceSettings {
  /// The patterns a switch keeps.
  pub fn kept(&self) -> impl Iterator<Item = &WarmPattern> {
    self
      .folders
      .iter()
      .filter(|folder| folder.choice == WarmChoice::Keep)
      .map(|folder| &folder.pattern)
  }

  /// The warm folder of `pattern`, if the project has it.
  pub fn folder(&self, pattern: &WarmPattern) -> Option<&WarmFolder> {
    self.folders.iter().find(|folder| folder.pattern == *pattern)
  }
}
