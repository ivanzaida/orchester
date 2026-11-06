//! Scoped invalidation subscriptions (S11). The server's `GET /api/v2/projects/{project}/subscription` streams
//! [`SubscriptionEvent`]s as server-sent events, each named by its `kind` with the cursor as its `id:`; a client
//! resumes with `Last-Event-ID`. The stream sends no snapshots: a client reads through the ordinary routes and drops a
//! cached read taken at cursor `R` only when an `invalidated` event with a cursor past `R` names its [`QueryScope`], so
//! it may subscribe before or after reading without missing a change.

use orchester_core::id::{ModuleId, ReleaseId, ResourceId, TaskId};
use serde::{Deserialize, Serialize};

use super::EventCursor;

/// A family of cached reads one committed change can make stale. `Ord` and `Hash` keep a set of scopes stable, so an
/// `invalidated` event names each scope once and in the same order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum QueryScope {
  /// The project, its policy, the caller's capabilities and the members.
  Workspace {},
  /// The task list and the task counts.
  TaskList {},
  Task {
    task: TaskId,
  },
  TaskComments {
    task: TaskId,
  },
  ModuleList {},
  /// A module's summary, progress, burndown and history.
  Module {
    module: ModuleId,
  },
  /// The library and its pickers.
  Library {},
  /// A resource's version list.
  Resource {
    resource: ResourceId,
  },
  /// The release list.
  ReleaseList {},
  /// A release's summary, progress, burndown, history and changelog.
  Release {
    release: ReleaseId,
  },
}

/// One server-sent event of a project subscription. Unit-like variants are braced so serde refuses unknown fields on
/// them too.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SubscriptionEvent {
  /// Sent first: the stream's position, with no snapshot.
  Ready { cursor: EventCursor },
  /// One committed batch: `cursor` is its last entry and `scopes` are de-duplicated.
  Invalidated {
    cursor: EventCursor,
    scopes: Vec<QueryScope>,
  },
  /// The resume point is below the replay floor: drop every cached read and read again at or after `cursor`.
  Reset { cursor: EventCursor },
  /// The caller lost access; the stream then closes.
  Revoked {},
}
