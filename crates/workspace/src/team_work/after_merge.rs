//! What *Merge and deliver* tells in the status bar (the person's decision of 2026-11-13): once a team task's run
//! merged into its module's branch, the module's delivery to its base is asked for in the same press, as the module's
//! page asks for it ([`super::delivery`]). The delivery is an operation of its own after the merge's: under way while
//! the server is asked and this computer merges, done once the module is delivered, left waiting for an Admin without
//! a failure, and failed with why it was not delivered.

use orchester_api::v2::AuthorityErrorCode;
use orchester_core::id::TaskId;

use crate::{
  activity::{OperationSpec, Place, Target},
  messages::Message,
  workspace::OpenProject,
};

/// Delivering team module `module` (its name) to its base, asked from the page of task `task`.
pub fn operation(open: &OpenProject, task: TaskId, module: &str) -> OperationSpec {
  let message = |text| Message::new(text).with("module", module.to_owned());
  OperationSpec::new(
    message("statusBar.operations.moduleDelivery.running"),
    message("statusBar.operations.moduleDelivery.done"),
    message("statusBar.operations.moduleDelivery.failed"),
  )
  .project(open.project.name().as_str())
  .target(Target::Project {
    repo_root: open.repo_root.clone(),
    place: Place::Task(task),
  })
}

/// How the operation ends while the delivery of `module` waits for an Admin: asked, and not a failure.
pub fn waits(module: &str) -> Message {
  Message::new("statusBar.operations.moduleDelivery.waits").with("module", module.to_owned())
}

/// Why a module was not delivered after the merge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotDelivered {
  /// Team runs of the module still work here: its branch is not done yet.
  WorkRunning,
  /// The checkout has no branch checked out to deliver onto, or has the module's own.
  NoBase,
  /// The module's branch is not in this checkout.
  BranchNotFound(String),
  /// This computer is not signed in to the team server.
  SignedOut,
  /// The server could not be reached, or its answer is not known yet.
  Unavailable,
  /// The server refused the module's claim or the delivery.
  Refused(AuthorityErrorCode),
  /// The server authorized it and the merge did not land here.
  NotMerged,
  /// The server authorized it, and its merge waits for other work in this checkout.
  MergeWaits,
  /// It could not be carried out here.
  Failed,
}

impl NotDelivered {
  /// The reason as the status bar says it under the failed operation.
  pub fn message(&self) -> Message {
    match self {
      Self::WorkRunning => Message::new("statusBar.operations.moduleDelivery.why.workRunning"),
      Self::NoBase => Message::new("statusBar.operations.moduleDelivery.why.noBase"),
      Self::BranchNotFound(branch) => {
        Message::new("statusBar.operations.moduleDelivery.why.branchNotFound").with("branch", branch.clone())
      }
      Self::SignedOut => Message::new("statusBar.operations.moduleDelivery.why.signedOut"),
      Self::Unavailable => Message::new("statusBar.operations.moduleDelivery.why.unavailable"),
      Self::Refused(code) => {
        Message::new("statusBar.operations.moduleDelivery.why.refused").with("code", code_name(*code))
      }
      Self::NotMerged => Message::new("statusBar.operations.moduleDelivery.why.notMerged"),
      Self::MergeWaits => Message::new("statusBar.operations.moduleDelivery.why.mergeWaits"),
      Self::Failed => Message::new("statusBar.operations.moduleDelivery.why.failed"),
    }
  }
}

/// The server's code by its snake_case name, which it serializes to.
fn code_name(code: AuthorityErrorCode) -> String {
  match serde_json::to_value(code) {
    Ok(serde_json::Value::String(name)) => name,
    // Every code is a unit variant that serializes to a string; one that did not is told by its Rust name.
    Ok(_) | Err(_) => format!("{code:?}"),
  }
}
