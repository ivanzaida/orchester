//! What uses a shared resource (`GET …/resources/{resource}/used-by`, `docs/cloud-api.md`; decision E of
//! `docs/design/team-configuration.md`): the admin reads it before archiving, to confirm which runs will stop (row 79
//! of `docs/architecture/server.md` stays: an archive still cancels them). A member may read it too. Both lists are
//! returned whole, with no paging: they are bounded by the project's resources and its runs that have not ended.

use orchester_core::{
  id::{ResourceId, RunId},
  resource::VersionNumber,
};
use serde::{Deserialize, Serialize};

use crate::{SharedResourceKind, UserRef};

/// The answer to `GET …/resources/{resource}/used-by`. Both lists are empty when nothing uses the resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsedBy {
  /// The other resources that are not archived and whose latest published version names this one, ordered by name
  /// compared case-insensitively and then by id. The resource asked about is never in it, even when its own latest
  /// version names itself. "Names" is decided by typed fields, not by a text scan of the body: `docs/cloud-api.md`
  /// lists them.
  pub resources: Vec<UsedByResource>,
  /// The runs that have not ended and whose execution bundle pins any version of this resource, ordered by the
  /// number part of the task key numerically (`KON-3` before `KON-12`) and then by run number. A run has not ended
  /// from `work.start` until the server accepts a terminal work report (`succeeded`, `failed` or `cancelled`); a run
  /// whose last report is `progress`, also one interrupted or waiting for a person on the desktop, has not ended.
  pub runs: Vec<UsedByRun>,
}

/// A resource whose latest published version names the resource asked about.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsedByResource {
  pub kind: SharedResourceKind,
  pub id: ResourceId,
  pub name: String,
  /// The latest version, the one that names it.
  pub version: VersionNumber,
}

/// A run that has not ended and whose execution bundle pins a version of the resource: the run an archive stops.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsedByRun {
  /// The key of the run's task, e.g. `KON-12`.
  pub task_key: String,
  pub run: RunId,
  /// The run's number within its task.
  pub number: u32,
  /// Who holds the task's claim now, and so who is told the run stopped; after a take-over it is the new holder.
  /// Absent when the claim was released.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub holder: Option<UserRef>,
}
