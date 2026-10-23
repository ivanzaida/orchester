//! The Deliver step a pipeline may end in (`docs/design/orchester-workspace.md` › *Delivery*): a run that passed
//! every step joins the project's merge queue, which syncs it with the base branch, checks the merged result again
//! and merges it, as the person approves it or on its own. A pipeline without one keeps the manual *Merge into
//! <base>* on the task page; its runs never join the queue. Pull request delivery is not a mode yet: the editor
//! shows it unavailable.

use serde::{Deserialize, Serialize};

use crate::id::ResourceId;

/// When a run that passed every step merges.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliverMode {
  /// The run waits in Needs you; a person reviews it and merges.
  #[default]
  ApproveToMerge,
  /// The run merges once every step and the check on the merged result passed.
  AutoMerge,
}

/// How the run's work lands on the base branch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMethod {
  /// The base branch gets a merge commit; the run's commits stay as they are.
  #[default]
  MergeCommit,
  /// The base branch gets one commit titled `<task key>: <task title>`.
  Squash,
}

/// The Deliver step: its mode, its merge method, the agent profile its Integrate step resolves conflicts with
/// (`None` until one is chosen: the step that implements the change does) and whether Check runs again on the merged
/// result, which Auto-merge always does.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "DeliverFields")]
pub struct DeliverStep {
  mode: DeliverMode,
  method: MergeMethod,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  integrator: Option<ResourceId>,
  recheck: bool,
}

/// A step added in the editor: Approve to merge with a merge commit, re-checking the merged result.
impl Default for DeliverStep {
  fn default() -> Self {
    Self::new(DeliverMode::default(), MergeMethod::default(), None, true)
  }
}

#[derive(Deserialize)]
struct DeliverFields {
  mode: DeliverMode,
  method: MergeMethod,
  #[serde(default)]
  integrator: Option<ResourceId>,
  recheck: bool,
}

impl From<DeliverFields> for DeliverStep {
  fn from(fields: DeliverFields) -> Self {
    Self::new(fields.mode, fields.method, fields.integrator, fields.recheck)
  }
}

impl DeliverStep {
  /// A Deliver step; Auto-merge re-checks the merged result whatever `recheck` says.
  pub fn new(mode: DeliverMode, method: MergeMethod, integrator: Option<ResourceId>, recheck: bool) -> Self {
    Self {
      mode,
      method,
      integrator,
      recheck: recheck || mode == DeliverMode::AutoMerge,
    }
  }

  pub const fn mode(&self) -> DeliverMode {
    self.mode
  }

  pub const fn method(&self) -> MergeMethod {
    self.method
  }

  /// The agent profile the Integrate step resolves conflicts with.
  pub const fn integrator(&self) -> Option<ResourceId> {
    self.integrator
  }

  /// Whether Check runs again on the merged result before the run merges.
  pub const fn recheck(&self) -> bool {
    self.recheck
  }

  /// Whether the re-check can be switched off: not for Auto-merge, which never merges an unchecked result.
  pub fn recheck_locked(&self) -> bool {
    self.mode == DeliverMode::AutoMerge
  }

  #[must_use]
  pub fn with_mode(self, mode: DeliverMode) -> Self {
    Self::new(mode, self.method, self.integrator, self.recheck)
  }

  #[must_use]
  pub fn with_method(self, method: MergeMethod) -> Self {
    Self { method, ..self }
  }

  #[must_use]
  pub fn with_integrator(self, integrator: Option<ResourceId>) -> Self {
    Self { integrator, ..self }
  }

  /// This step re-checking the merged result or not; Auto-merge keeps it on.
  #[must_use]
  pub fn with_recheck(self, recheck: bool) -> Self {
    Self::new(self.mode, self.method, self.integrator, recheck)
  }
}
