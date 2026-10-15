use serde::{Deserialize, Serialize};

use super::LocalPath;

/// What the repository's commit hooks did that the person did not review.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HookChange {
  /// Orchester stopped the commit before moving the branch: the hooks changed the committed content of `paths`,
  /// and rewrote the reviewed message if `message`. Nothing was committed; the checkout, its index and the branch
  /// are as reviewed.
  Stopped { paths: Vec<LocalPath>, message: bool },
  /// The reviewed `commit` landed, then a post-commit hook moved HEAD to `head`.
  MovedHead { commit: String, head: String },
}
