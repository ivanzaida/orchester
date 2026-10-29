//! A shared project's event stream (`docs/architecture/server.md`, "Events", row 73): a paged snapshot on connect for a
//! client without a cursor or with one older than the retention, then deltas that say who did what. Each SSE event's
//! name is its [`EventKind`] and its data the kind's payload; [`Event`] reads and writes them as one
//! `{ "kind", "data" }` value.

use orchester_core::{
  id::{ModuleId, ProjectId, ResourceId, TagId, TaskId, UserId},
  run::TaskPhase,
  sharing::TaskRevision,
  task::{
    TaskStatus,
    organization::{ChangeActor, Priority, WorkflowStatus},
  },
};
use serde::{Deserialize, Serialize};

use crate::{
  Blocking, Member, MemberRole, SharedClaim, SharedLimits, SharedModule, SharedProject, SharedResource, SharedTag,
  SharedTask, UserRef, tasks::present, wire,
};

/// The kinds of the project stream, by their SSE event names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventKind {
  #[serde(rename = "snapshot.begin")]
  SnapshotBegin,
  #[serde(rename = "snapshot.tasks")]
  SnapshotTasks,
  #[serde(rename = "snapshot.end")]
  SnapshotEnd,
  #[serde(rename = "task.created")]
  TaskCreated,
  #[serde(rename = "task.changed")]
  TaskChanged,
  #[serde(rename = "task.claim")]
  TaskClaim,
  #[serde(rename = "task.run")]
  TaskRun,
  #[serde(rename = "tag.created")]
  TagCreated,
  #[serde(rename = "tag.renamed")]
  TagRenamed,
  #[serde(rename = "tag.deleted")]
  TagDeleted,
  #[serde(rename = "tag.merged")]
  TagMerged,
  #[serde(rename = "module.created")]
  ModuleCreated,
  #[serde(rename = "module.changed")]
  ModuleChanged,
  #[serde(rename = "member.added")]
  MemberAdded,
  #[serde(rename = "member.removed")]
  MemberRemoved,
  #[serde(rename = "member.role_changed")]
  MemberRoleChanged,
  #[serde(rename = "project.changed")]
  ProjectChanged,
  #[serde(rename = "resource.published")]
  ResourcePublished,
  #[serde(rename = "resource.renamed")]
  ResourceRenamed,
  #[serde(rename = "resource.archived")]
  ResourceArchived,
  #[serde(rename = "revoked")]
  Revoked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownEventKind;

impl EventKind {
  pub const ALL: [Self; 21] = [
    Self::SnapshotBegin,
    Self::SnapshotTasks,
    Self::SnapshotEnd,
    Self::TaskCreated,
    Self::TaskChanged,
    Self::TaskClaim,
    Self::TaskRun,
    Self::TagCreated,
    Self::TagRenamed,
    Self::TagDeleted,
    Self::TagMerged,
    Self::ModuleCreated,
    Self::ModuleChanged,
    Self::MemberAdded,
    Self::MemberRemoved,
    Self::MemberRoleChanged,
    Self::ProjectChanged,
    Self::ResourcePublished,
    Self::ResourceRenamed,
    Self::ResourceArchived,
    Self::Revoked,
  ];

  pub const fn as_str(self) -> &'static str {
    match self {
      Self::SnapshotBegin => "snapshot.begin",
      Self::SnapshotTasks => "snapshot.tasks",
      Self::SnapshotEnd => "snapshot.end",
      Self::TaskCreated => "task.created",
      Self::TaskChanged => "task.changed",
      Self::TaskClaim => "task.claim",
      Self::TaskRun => "task.run",
      Self::TagCreated => "tag.created",
      Self::TagRenamed => "tag.renamed",
      Self::TagDeleted => "tag.deleted",
      Self::TagMerged => "tag.merged",
      Self::ModuleCreated => "module.created",
      Self::ModuleChanged => "module.changed",
      Self::MemberAdded => "member.added",
      Self::MemberRemoved => "member.removed",
      Self::MemberRoleChanged => "member.role_changed",
      Self::ProjectChanged => "project.changed",
      Self::ResourcePublished => "resource.published",
      Self::ResourceRenamed => "resource.renamed",
      Self::ResourceArchived => "resource.archived",
      Self::Revoked => "revoked",
    }
  }

  /// Snapshot pages carry no SSE `id:`: the cursor moves only at `snapshot.end` and with each delta.
  pub const fn is_snapshot(self) -> bool {
    matches!(self, Self::SnapshotBegin | Self::SnapshotTasks | Self::SnapshotEnd)
  }
}

impl TryFrom<&str> for EventKind {
  type Error = UnknownEventKind;
  fn try_from(value: &str) -> Result<Self, Self::Error> {
    Self::ALL
      .into_iter()
      .find(|kind| kind.as_str() == value)
      .ok_or(UnknownEventKind)
  }
}

/// One event of the stream: its kind and its payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum Event {
  #[serde(rename = "snapshot.begin")]
  SnapshotBegin(SnapshotBegin),
  #[serde(rename = "snapshot.tasks")]
  SnapshotTasks(SnapshotTasks),
  #[serde(rename = "snapshot.end")]
  SnapshotEnd(SnapshotEnd),
  #[serde(rename = "task.created")]
  TaskCreated(Delta<TaskCreated>),
  #[serde(rename = "task.changed")]
  TaskChanged(Delta<TaskChanged>),
  #[serde(rename = "task.claim")]
  TaskClaim(Delta<TaskClaim>),
  #[serde(rename = "task.run")]
  TaskRun(Delta<TaskRun>),
  #[serde(rename = "tag.created")]
  TagCreated(Delta<TagCreated>),
  #[serde(rename = "tag.renamed")]
  TagRenamed(Delta<TagRenamed>),
  #[serde(rename = "tag.deleted")]
  TagDeleted(Delta<TagDeleted>),
  #[serde(rename = "tag.merged")]
  TagMerged(Delta<TagMerged>),
  #[serde(rename = "module.created")]
  ModuleCreated(Delta<ModuleCreated>),
  #[serde(rename = "module.changed")]
  ModuleChanged(Delta<ModuleChanged>),
  #[serde(rename = "member.added")]
  MemberAdded(Delta<MemberAdded>),
  #[serde(rename = "member.removed")]
  MemberRemoved(Delta<MemberRemoved>),
  #[serde(rename = "member.role_changed")]
  MemberRoleChanged(Delta<MemberRoleChanged>),
  #[serde(rename = "project.changed")]
  ProjectChanged(Delta<ProjectChanged>),
  #[serde(rename = "resource.published")]
  ResourcePublished(Delta<ResourcePublished>),
  #[serde(rename = "resource.renamed")]
  ResourceRenamed(Delta<ResourceRenamed>),
  #[serde(rename = "resource.archived")]
  ResourceArchived(Delta<ResourceArchived>),
  #[serde(rename = "revoked")]
  Revoked(Revoked),
}

impl Event {
  pub const fn kind(&self) -> EventKind {
    match self {
      Self::SnapshotBegin(_) => EventKind::SnapshotBegin,
      Self::SnapshotTasks(_) => EventKind::SnapshotTasks,
      Self::SnapshotEnd(_) => EventKind::SnapshotEnd,
      Self::TaskCreated(_) => EventKind::TaskCreated,
      Self::TaskChanged(_) => EventKind::TaskChanged,
      Self::TaskClaim(_) => EventKind::TaskClaim,
      Self::TaskRun(_) => EventKind::TaskRun,
      Self::TagCreated(_) => EventKind::TagCreated,
      Self::TagRenamed(_) => EventKind::TagRenamed,
      Self::TagDeleted(_) => EventKind::TagDeleted,
      Self::TagMerged(_) => EventKind::TagMerged,
      Self::ModuleCreated(_) => EventKind::ModuleCreated,
      Self::ModuleChanged(_) => EventKind::ModuleChanged,
      Self::MemberAdded(_) => EventKind::MemberAdded,
      Self::MemberRemoved(_) => EventKind::MemberRemoved,
      Self::MemberRoleChanged(_) => EventKind::MemberRoleChanged,
      Self::ProjectChanged(_) => EventKind::ProjectChanged,
      Self::ResourcePublished(_) => EventKind::ResourcePublished,
      Self::ResourceRenamed(_) => EventKind::ResourceRenamed,
      Self::ResourceArchived(_) => EventKind::ResourceArchived,
      Self::Revoked(_) => EventKind::Revoked,
    }
  }
}

/// The snapshot's first page: everything but the tasks. `project` is as the connected member reads it, with their role.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotBegin {
  pub project: SharedProject,
  pub members: Vec<Member>,
  pub tags: Vec<SharedTag>,
  pub modules: Vec<SharedModule>,
  /// The project's resources, each with its latest version number; empty until S5.
  #[serde(default)]
  pub resources: Vec<SharedResource>,
}

/// At most [`SnapshotTasks::PAGE`] tasks; as many pages as the project needs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotTasks {
  pub tasks: Vec<SharedTask>,
}

impl SnapshotTasks {
  pub const PAGE: usize = 64;
}

/// The snapshot's last page: the sequence of the last event it includes, from which the deltas follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotEnd {
  pub cursor: u64,
}

/// A delta: its sequence `id`, who made the change and whether Orchester acted for them, when, and its kind's fields.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Delta<T> {
  pub id: u64,
  pub by: UserRef,
  #[serde(with = "wire::Actor")]
  pub actor: ChangeActor,
  pub at: i64,
  #[serde(flatten)]
  pub change: T,
}

/// A create, an import, or an upload at Share or Join.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskCreated {
  pub task: Box<SharedTask>,
}

/// Only the fields that changed, made on `base` and now at `revision`; a client whose cached revision is not `base`
/// reads the task again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskChanged {
  pub task: TaskId,
  pub revision: TaskRevision,
  pub base: TaskRevision,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub brief: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub criteria: Option<Vec<String>>,
  #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
  pub pipeline: Option<Option<String>>,
  #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
  pub limits: Option<Option<SharedLimits>>,
  #[serde(default, with = "wire::optional_status", skip_serializing_if = "Option::is_none")]
  pub status: Option<TaskStatus>,
  #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
  pub module: Option<Option<ModuleId>>,
  #[serde(
    default,
    with = "wire::optional_priority_wire",
    skip_serializing_if = "Option::is_none"
  )]
  pub priority: Option<Priority>,
  #[serde(default, with = "wire::optional_workflow", skip_serializing_if = "Option::is_none")]
  pub workflow: Option<WorkflowStatus>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub tags_added: Vec<TagId>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub tags_removed: Vec<TagId>,
  /// The task's blocking links as they are now.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub blocked_by: Option<Vec<Blocking>>,
  #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
  pub blocked_reason: Option<Option<String>>,
}

/// What happened to a task's claim. `stale` and `refreshed` come from the staleness sweep and a heartbeat, with the
/// actor `orchester` and the holder as `by`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimAction {
  Taken,
  Released,
  TakenOver,
  /// A merge completed it.
  Completed,
  Stale,
  Refreshed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskClaim {
  pub task: TaskId,
  /// The claim as it is now.
  pub claim: Option<SharedClaim>,
  pub action: ClaimAction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStep {
  pub key: String,
  pub name: String,
}

/// A run's progress, light: the rest of the task's run is read with the task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRun {
  pub task: TaskId,
  pub number: u32,
  #[serde(with = "wire::Phase")]
  pub phase: TaskPhase,
  pub step: Option<RunStep>,
  pub phase_since: i64,
  pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TagCreated {
  pub tag: SharedTag,
}

/// The tag with its new name.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TagRenamed {
  pub tag: SharedTag,
}

/// Every task the tag is removed from sends its own [`TaskChanged`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagDeleted {
  pub tag: TagId,
}

/// `from` goes away into `into`; every rewritten task sends its own [`TaskChanged`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TagMerged {
  pub from: TagId,
  pub into: SharedTag,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleCreated {
  pub module: SharedModule,
}

/// A module change replaces its fields as a whole; a task's move is its [`TaskChanged::module`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleChanged {
  pub module: SharedModule,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemberAdded {
  pub member: Member,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberRemoved {
  pub user: UserId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberRoleChanged {
  pub user: UserId,
  pub role: MemberRole,
}

/// Project events carry common metadata, never the acting member's personal role.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectChanged {
  pub project: ProjectNames,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectNames {
  pub id: ProjectId,
  pub key: String,
  pub name: String,
}

/// The resource's metadata with the new version number as `latest`; the body is read with `GET …/versions`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourcePublished {
  pub resource: SharedResource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRenamed {
  pub resource: ResourceId,
  pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceArchived {
  pub resource: ResourceId,
}

/// The session or this person's membership ended; the stream closes after it. Not a delta: it is the connection's,
/// not a recorded change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revoked {}
