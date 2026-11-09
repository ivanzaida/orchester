//! How and by whom a team task was closed (Done or Canceled), as its read says it (decision D4,
//! `docs/cloud-api.md` "Team authority task lifecycle"). A task's `updated` is its last change by anyone; this is the
//! change that closed it, which a later edit of the title does not replace.

use orchester_core::id::{ModuleId, WorkId};
use serde::{Deserialize, Serialize};

use crate::UserRef;

/// How a task came to be Done or Canceled, and when.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskClosure {
  pub how: ClosedHow,
  /// When the server closed the task, in milliseconds since the Unix epoch.
  pub at: i64,
}

/// What closed a task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClosedHow {
  /// A person's task patch: Done by the holder of the task's claim or an Admin, Canceled by an Admin. `by` is the
  /// caller, whoever held the claim.
  Person { by: UserRef },
  /// The accepted delivery of the task's module into base marked it Done. `module_name` is the module's name when the
  /// delivery was accepted.
  Delivery { module: ModuleId, module_name: String },
  /// The success of the task's work marked it Done, as the work delivers no changes to base; `work` is that work.
  Run { work: WorkId },
}
