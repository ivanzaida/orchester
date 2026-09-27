//! How large a change is, and the largest change Orchester reads as a diff. GitHub's own diff
//! limits are of the same order; a larger change is reviewed elsewhere (on GitHub, or with git).

use serde::{Deserialize, Serialize};

/// Changed lines (added plus removed) of the largest diff Orchester reads.
pub const MAX_DIFF_LINES: u32 = 20_000;
/// Changed files of the largest diff Orchester reads.
pub const MAX_DIFF_FILES: u32 = 300;

/// Size of a change: files, added and removed lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changes {
  pub files: u32,
  pub additions: u32,
  pub deletions: u32,
}

impl Changes {
  /// Whether the change is larger than the diffs Orchester reads.
  pub const fn is_too_large(self) -> bool {
    self.additions.saturating_add(self.deletions) > MAX_DIFF_LINES || self.files > MAX_DIFF_FILES
  }
}
