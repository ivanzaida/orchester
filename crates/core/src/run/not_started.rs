//! Why a run's worktree could not be created, so the run never started
//! ([`RunFailure::WorktreeNotCreated`](super::RunFailure::WorktreeNotCreated)). The reason is kept with the run, shown
//! to the person and handed to automation, so it names the cause in words of its own; git's own message is kept only
//! where nothing else names it, without credentials and shortened. The runner's log has the full detail.

use std::iter;

use serde::{Deserialize, Serialize};

use crate::repository::redact_userinfo;

/// Why the run's worktree could not be created.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeFailure {
  /// `.orchester/` is not ignored by git in the repository: a worktree there would show in the person's checkout.
  NotIgnored,
  /// The run's branch name and each of its numbered names are taken.
  NoFreeBranch,
  /// Git could not be started.
  GitUnavailable,
  /// The run's base does not name a commit of the repository.
  UnknownBase,
  /// Git does not accept the run's branch name.
  InvalidBranch,
  /// Something that is not a worktree of the repository on a branch is where the run's worktree goes.
  FolderInTheWay,
  /// Git refused, in its own words.
  Git(GitMessage),
  /// Something else failed; only the runner's log says what.
  Unexpected,
}

/// What git said when it refused to create a run's worktree, as far as it is kept: credentials in URLs removed,
/// its last [`GitMessage::MAX_LINES`] lines that say something, and of them the last [`GitMessage::MAX_CHARS`]
/// characters. A hook or filter that runs while git checks out can print anything, at any length.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub struct GitMessage(String);

impl GitMessage {
  pub const MAX_LINES: usize = 4;
  pub const MAX_CHARS: usize = 400;

  /// `text`, git's error output, as far as it is kept. Keeping what was kept already changes nothing.
  pub fn new(text: &str) -> Self {
    let redacted = redact_userinfo(text);
    let lines: Vec<&str> = redacted
      .lines()
      .map(str::trim_end)
      .filter(|line| !line.trim().is_empty())
      .collect();
    let kept = lines[lines.len().saturating_sub(Self::MAX_LINES)..].join("\n");
    if kept.chars().count() <= Self::MAX_CHARS {
      return Self(kept);
    }
    // Git ends with what failed, so its end is kept.
    let skipped = kept.chars().count() - (Self::MAX_CHARS - 1);
    Self(iter::once('…').chain(kept.chars().skip(skipped)).collect())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// A stored or received message is kept as a new one would be, so none reaches a person unbounded.
impl From<String> for GitMessage {
  fn from(text: String) -> Self {
    Self::new(&text)
  }
}

impl From<GitMessage> for String {
  fn from(message: GitMessage) -> Self {
    message.0
  }
}
