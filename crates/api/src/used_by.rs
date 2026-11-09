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
  /// The runs that have not ended and whose execution bundle pins any version of this resource, ordered by the number
  /// part of the task key numerically (`KON-3` before `KON-12`) and then by run id (a stable tie-break). Only each
  /// task's current work is listed, the latest work started under the task's current claim: the server fences a work
  /// after a take-over, a release or a restart under the same claim and accepts no further report from it, so its run
  /// could never end. A released task lists none; a work whose bundle is an older schema (its pins are not known) is
  /// not listed (a known limit). The server learns a run only from its work reports, so a run is listed from the first
  /// work report the server accepted for it until a terminal one (`succeeded`, `failed` or `cancelled`); work started
  /// and not reported yet is not listed (a known limit). A run whose last report is `progress`, also one interrupted
  /// or waiting for a person on the desktop, has not ended, unless a run whose first accepted report came after its
  /// own exists for the same work: that run has ended, superseded. Runs are ordered by the server's acceptance of
  /// their first report (the order of `WorkRead.runs`), never by run id; a run whose first report arrives late is the
  /// newest.
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

/// A run that has not ended and whose execution bundle pins a version of the resource: the run an archive stops. The
/// server has no run number (a work report carries only the run's id).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsedByRun {
  /// The key of the run's task, e.g. `KON-12`.
  pub task_key: String,
  pub run: RunId,
  /// Who holds the task's claim now, and so who is told the run stopped; after a take-over it is the new holder. The
  /// team server always sets it, since only current work under the current claim is listed; it is absent only for a
  /// local project's runs, which have no claim holder.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub holder: Option<UserRef>,
}
