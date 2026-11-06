//! What uses a shared resource (`GET …/resources/{resource}/used-by`, `docs/cloud-api.md`; decision E of
//! `docs/design/team-configuration.md`): the admin reads it before archiving, to confirm which runs will stop (row 79
//! of `docs/architecture/server.md` stays: an archive still cancels them). A member may read it too.

use orchester_core::{
  id::{ResourceId, RunId},
  resource::VersionNumber,
};
use serde::{Deserialize, Serialize};

use crate::{SharedResourceKind, UserRef};

/// The answer to `GET …/resources/{resource}/used-by`. Both lists are empty when nothing uses the resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsedBy {
  /// The other resources that are not archived and whose latest published version names this one in its body or
  /// references, by name and then id.
  pub resources: Vec<UsedByResource>,
  /// The runs that have not ended and pin any version of this resource, by task key and then run number.
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

/// A run that has not ended and pins a version of the resource: the run an archive stops.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsedByRun {
  /// The key of the run's task, e.g. `KON-12`.
  pub task_key: String,
  pub run: RunId,
  /// The run's number within its task.
  pub number: u32,
  /// Who holds the task's claim, and so who is told the run stopped.
  pub holder: UserRef,
}
