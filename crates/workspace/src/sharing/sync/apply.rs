//! Applying a shared project's events to its cache (`docs/architecture/server.md`, "Cursor and applying events"): the
//! snapshot's pages replace the confirmed cache at `snapshot.end`; each delta is applied with core's rules
//! ([`CachedTask::apply`], [`remap_tags`]) and the cursor advances in the same transaction. A task delta the cache
//! cannot take asks for that one task to be read again, and a published resource for its new versions
//! ([`super::config`]). Members are read when Sharing shows them, and a resource's rename and archived mark are not
//! kept apart from a version yet: their deltas only move the cursor. Blocking: the stream runs these on the blocking
//! pool.

use std::{num::NonZeroU32, path::Path};

use orchester_api::{
  Delta, Event, SharedResource, SharedTask, SnapshotBegin, TaskChanged, TaskClaim, TaskRun, client::StreamEnd,
};
use orchester_core::{
  id::TaskId,
  name::Name,
  sharing::{CachedTask, DeltaOutcome, RunProgress, TagRemap, TaskChange, TaskDelta, TaskDeltaKind, remap_tags},
  task::{TaskLimits, organization::BlockingTask},
  time::Timestamp,
};
use orchester_store::{ProjectDb, SharedEvent, SharedSnapshot, StoreError};

use crate::sharing::{
  SharingError,
  project::{convert, organization::tag},
};

/// The snapshot's pages, kept aside until its end: a stream that breaks before leaves the cache as it was.
pub(super) struct Pages {
  begin: SnapshotBegin,
  tasks: Vec<SharedTask>,
}

impl Pages {
  pub(super) const fn new(begin: SnapshotBegin) -> Self {
    Self {
      begin,
      tasks: Vec::new(),
    }
  }

  pub(super) fn extend(&mut self, tasks: Vec<SharedTask>) {
    self.tasks.extend(tasks);
  }

  /// The project's resources, each with its latest version number.
  pub(super) fn resources(&self) -> &[SharedResource] {
    &self.begin.resources
  }
}

/// What a delta did to the cache.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Applied {
  Changed,
  /// The cache had it already; only the cursor moved.
  Unchanged,
  /// The cache cannot take it: `task` is read again and written with the cursor `cursor`.
  Read {
    task: TaskId,
    cursor: u64,
  },
  /// `resource` was published: its new versions are read, then the cursor moves to `cursor`.
  Published {
    resource: Box<SharedResource>,
    cursor: u64,
  },
}

/// Where the stream continues: `None` the first time (no event has sequence 0), which gets the snapshot.
pub(super) fn cursor(root: &Path) -> Result<Option<u64>, SharingError> {
  let link = ProjectDb::open(root)?.sharing()?.ok_or(StoreError::NotShared)?;
  Ok(Some(link.cursor).filter(|cursor| *cursor > 0))
}

/// Replaces the cache with the snapshot `pages` ended at `cursor`.
pub(super) fn snapshot(root: &Path, pages: &Pages, cursor: u64) -> Result<(), SharingError> {
  let snapshot = SharedSnapshot {
    role: pages.begin.project.role,
    tags: pages.begin.tags.iter().map(tag).collect::<Result<_, _>>()?,
    modules: pages
      .begin
      .modules
      .iter()
      .map(convert::cached_module)
      .collect::<Result<_, _>>()?,
    tasks: pages.tasks.iter().map(convert::cached).collect::<Result<_, _>>()?,
    cursor,
  };
  ProjectDb::open(root)?.take_shared_snapshot(&snapshot, Timestamp::now())?;
  Ok(())
}

/// Applies the delta `event`; a snapshot page or `revoked` is no delta.
pub(super) fn delta(root: &Path, event: &Event) -> Result<Applied, SharingError> {
  let mut db = ProjectDb::open(root)?;
  let (cursor, change) = match event {
    Event::TaskCreated(delta) => (delta.id, Some(tasks([convert::cached(&delta.change.task)?]))),
    Event::TaskChanged(delta) => (
      delta.id,
      task_delta(&db, delta, changed(&delta.change)?, delta.change.task)?,
    ),
    Event::TaskClaim(delta) => (
      delta.id,
      task_delta(&db, delta, claimed(&delta.change), delta.change.task)?,
    ),
    Event::TaskRun(delta) => (
      delta.id,
      task_delta(&db, delta, progressed(&delta.change)?, delta.change.task)?,
    ),
    Event::TagCreated(delta) => (delta.id, Some(tagged(&delta.change.tag)?)),
    Event::TagRenamed(delta) => (delta.id, Some(tagged(&delta.change.tag)?)),
    Event::TagDeleted(delta) => (
      delta.id,
      Some(remapped(&db, TagRemap::Deleted(delta.change.tag), None)?),
    ),
    Event::TagMerged(delta) => {
      let remap = TagRemap::Merged {
        from: delta.change.from,
        into: delta.change.into.id,
      };
      (delta.id, Some(remapped(&db, remap, Some(&delta.change.into))?))
    }
    Event::ModuleCreated(delta) => (delta.id, Some(module(&delta.change.module)?)),
    Event::ModuleChanged(delta) => (delta.id, Some(module(&delta.change.module)?)),
    Event::ResourcePublished(delta) => {
      return Ok(Applied::Published {
        resource: Box::new(delta.change.resource.clone()),
        cursor: delta.id,
      });
    }
    Event::MemberAdded(Delta { id, .. })
    | Event::MemberRemoved(Delta { id, .. })
    | Event::MemberRoleChanged(Delta { id, .. })
    | Event::ProjectChanged(Delta { id, .. })
    | Event::ResourceRenamed(Delta { id, .. })
    | Event::ResourceArchived(Delta { id, .. }) => (*id, None),
    Event::SnapshotBegin(_) | Event::SnapshotTasks(_) | Event::SnapshotEnd(_) | Event::Revoked(_) => {
      return Err(SharingError::InvalidAnswer("event"));
    }
  };
  let Some(change) = change else {
    if let Some(task) = event_task(event) {
      return Ok(Applied::Read { task, cursor });
    }
    db.take_shared_event(&SharedEvent::default(), cursor, Timestamp::now())?;
    return Ok(Applied::Unchanged);
  };
  let applied = if change == SharedEvent::default() {
    Applied::Unchanged
  } else {
    Applied::Changed
  };
  db.take_shared_event(&change, cursor, Timestamp::now())?;
  Ok(applied)
}

/// Writes `task`, read again after a delta the cache could not take, with the cursor that delta moves to.
pub(super) fn read(root: &Path, task: &SharedTask, cursor: u64) -> Result<(), SharingError> {
  let event = tasks([convert::cached(task)?]);
  ProjectDb::open(root)?.take_shared_event(&event, cursor, Timestamp::now())?;
  Ok(())
}

/// Moves the cursor past a delta whose change is written already or that has nothing to write: one of a task the server
/// no longer has, or a published resource [`super::config`] kept.
pub(super) fn skip(root: &Path, cursor: u64) -> Result<(), SharingError> {
  ProjectDb::open(root)?.take_shared_event(&SharedEvent::default(), cursor, Timestamp::now())?;
  Ok(())
}

/// The server ended the person's membership (`403`, `revoked`): the list reads *not a member* since now.
pub(super) fn not_member(root: &Path) -> Result<(), SharingError> {
  let mut db = ProjectDb::open(root)?;
  let Some(link) = db.sharing()? else {
    return Ok(());
  };
  if link.removed_at.is_none() {
    db.record_sharing_state(link.role, None, Some(Timestamp::now()))?;
  }
  Ok(())
}

/// Whether `end` ends the person's membership.
pub(super) const fn ends_membership(end: &StreamEnd) -> bool {
  matches!(end, StreamEnd::Revoked | StreamEnd::Forbidden)
}

/// `kind` applied to the cached task `task`: the write, `Some(empty)` when the cache has it already, or `None` when
/// the task is read again.
fn task_delta<T>(
  db: &ProjectDb,
  delta: &Delta<T>,
  kind: TaskDeltaKind,
  task: TaskId,
) -> Result<Option<SharedEvent>, SharingError> {
  let Some(cached) = db.cached_task(task)? else {
    return Ok(None);
  };
  let delta = TaskDelta {
    by: convert::person(&delta.by),
    at: Timestamp::from_unix_millis(delta.at),
    change: kind,
  };
  Ok(match cached.apply(&delta) {
    DeltaOutcome::Applied(task) => Some(tasks([*task])),
    DeltaOutcome::AlreadyHave => Some(SharedEvent::default()),
    DeltaOutcome::Stale => None,
  })
}

fn event_task(event: &Event) -> Option<TaskId> {
  match event {
    Event::TaskChanged(delta) => Some(delta.change.task),
    Event::TaskClaim(delta) => Some(delta.change.task),
    Event::TaskRun(delta) => Some(delta.change.task),
    _ => None,
  }
}

fn tasks<const N: usize>(tasks: [CachedTask; N]) -> SharedEvent {
  SharedEvent {
    tasks: tasks.into(),
    ..SharedEvent::default()
  }
}

fn tagged(shared: &orchester_api::SharedTag) -> Result<SharedEvent, SharingError> {
  Ok(SharedEvent {
    tags: vec![tag(shared)?],
    ..SharedEvent::default()
  })
}

/// `remap` of every cached task that has the tag going away; a merge also writes the tag it goes `into`.
fn remapped(
  db: &ProjectDb,
  remap: TagRemap,
  into: Option<&orchester_api::SharedTag>,
) -> Result<SharedEvent, SharingError> {
  let gone = match remap {
    TagRemap::Deleted(tag) | TagRemap::Merged { from: tag, .. } => tag,
  };
  Ok(SharedEvent {
    tasks: remap_tags(&db.cached_tasks()?, remap),
    tags: into.map(tag).transpose()?.into_iter().collect(),
    removed_tags: vec![gone],
    modules: Vec::new(),
  })
}

fn module(module: &orchester_api::SharedModule) -> Result<SharedEvent, SharingError> {
  Ok(SharedEvent {
    modules: vec![convert::cached_module(module)?],
    ..SharedEvent::default()
  })
}

fn changed(changed: &TaskChanged) -> Result<TaskDeltaKind, SharingError> {
  let mut change = TaskChange::new(changed.base, changed.revision);
  change.title = changed
    .title
    .as_deref()
    .map(Name::new)
    .transpose()
    .map_err(|_| SharingError::InvalidAnswer("task title"))?;
  change.brief.clone_from(&changed.brief);
  change.criteria.clone_from(&changed.criteria);
  change.pipeline.clone_from(&changed.pipeline);
  change.limits = changed
    .limits
    .as_ref()
    .map(|limits| limits.as_ref().map_or(Ok(TaskLimits::NONE), convert::limits))
    .transpose()?;
  change.status = changed.status;
  change.module = changed.module;
  change.priority = changed.priority;
  change.workflow = changed.workflow;
  change.tags_added.clone_from(&changed.tags_added);
  change.tags_removed.clone_from(&changed.tags_removed);
  change.blocked_by = changed.blocked_by.as_ref().map(|links| {
    links
      .iter()
      .map(|link| BlockingTask {
        task: link.task,
        gate: link.gate,
      })
      .collect()
  });
  change.blocked_reason.clone_from(&changed.blocked_reason);
  Ok(TaskDeltaKind::Changed(change))
}

fn claimed(claim: &TaskClaim) -> TaskDeltaKind {
  TaskDeltaKind::Claim(claim.claim.as_ref().map(convert::claim))
}

/// The cache names a run's step by its name, as the holder reported it.
fn progressed(run: &TaskRun) -> Result<TaskDeltaKind, SharingError> {
  Ok(TaskDeltaKind::Run(RunProgress {
    number: NonZeroU32::new(run.number).ok_or(SharingError::InvalidAnswer("run number"))?,
    phase: run.phase,
    step: run.step.as_ref().map(|step| step.name.clone()),
    phase_since: Timestamp::from_unix_millis(run.phase_since),
    updated_at: Timestamp::from_unix_millis(run.updated_at),
  }))
}
