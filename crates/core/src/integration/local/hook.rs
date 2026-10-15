use serde::{Deserialize, Serialize};

use super::LocalPath;

/// What the repository's commit hooks changed when Orchester stopped a commit before moving the branch: the
/// paths whose committed content they changed, and whether they rewrote the reviewed message. Nothing was
/// committed; the checkout, its index and the branch are as reviewed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookChange {
  pub paths: Vec<LocalPath>,
  pub message: bool,
}
