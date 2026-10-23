//! Driving a project's queue, one step at a time ([`advance`]): runs that passed every step join it, then the run
//! being delivered, else the first one waiting whose run passed every step, moves on. Queued, it syncs with its base
//! branch; synced cleanly, Check runs again on the merged result, or its conflicts go to the Integrate step; checked on
//! the base as it still is, it waits for a person's approval (which does not hold the queue) or merges. A base that
//! moved during the check means syncing and checking again, at most [`MAX_RESYNCS`] times in a row. A checkout that
//! keeps the merge out blocks the queue until it changes. Whatever stops a run takes it out of the queue with the
//! reason, and nothing of it reaches the base branch.

use std::{
  collections::HashSet,
  path::{Path, PathBuf},
  sync::LazyLock,
};

use orchester_core::{
  id::RunId,
  integration::{
    branch_head,
    delivery::{self, Synced},
  },
  merge_queue::{CheckedOn, Left, MAX_RESYNCS, QueueEntry, QueueEvent, QueueState},
  resource::{DeliverMode, DeliverStep},
  revision::Revision,
  run::{Run, RunStatus},
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_ipc::Command;
use orchester_store::{ProjectDb, StoreError};
use parking_lot::Mutex;

use super::{
  check::{Checked, check},
  integrate::{integrate, integrate_step},
  join::join,
  land::{Landed, land},
  wake,
};
use crate::{
  messages::{self, Message},
  tasks::{
    changes::{GIT, run_worktree},
    detail::run_store_error,
  },
};

/// What [`advance`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Advance {
  /// Nothing to do until the runs, the queue or a person change something; with `poll`, also once the person's
  /// checkout may have changed: a blocked delivery goes on by itself once it is clean.
  Idle { poll: bool },
  /// The queue moved on: advance again.
  Changed,
  /// `run`'s conflicts go to its Integrate step: send `command` to the project's runner, then tell how that went
  /// with [`integrate_sent`].
  Integrate { run: RunId, command: Command },
}

/// The projects whose queue moves on now: one step at a time per project, whoever asks.
static DRIVING: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Default::default);

/// Holds a project's queue while it moves on.
struct Driving(PathBuf);

impl Driving {
  fn take(root: &Path) -> Option<Self> {
    DRIVING
      .lock()
      .insert(root.to_path_buf())
      .then(|| Self(root.to_path_buf()))
  }
}

impl Drop for Driving {
  fn drop(&mut self) {
    DRIVING.lock().remove(&self.0);
  }
}

/// Moves the queue of the project at `root` on by one step, with `programs` running git and the checks. While
/// another caller moves it on, this does nothing. Blocks on SQLite, git and the checks.
pub fn advance(root: &Path, programs: &Programs) -> Result<Advance, Message> {
  let Some(_driving) = Driving::take(root) else {
    return Ok(Advance::Idle { poll: false });
  };
  let advanced = step(root, programs).map_err(|error| {
    tracing::warn!(%error, "the merge queue cannot move on");
    messages::store_error(&error)
  })?;
  if !matches!(advanced, Advance::Idle { .. }) {
    wake(root);
  }
  Ok(advanced)
}

fn step(root: &Path, programs: &Programs) -> Result<Advance, StoreError> {
  let mut db = ProjectDb::open(root)?;
  if join(&mut db, Timestamp::now())? {
    return Ok(Advance::Changed);
  }
  let Some((entry, run)) = next(&db)? else {
    return Ok(Advance::Idle { poll: false });
  };
  Driver { db, root, programs }.step(entry, run)
}

/// The entry that moves on now, with its run: the one being delivered, else the first waiting one whose run passed
/// every step (one that works again, after a follow-up say, waits until it passed again).
fn next(db: &ProjectDb) -> Result<Option<(QueueEntry, Option<Run>)>, StoreError> {
  let entries = db.merge_queue()?;
  let delivering = entries.iter().find(|entry| entry.state.delivering());
  let candidates = delivering.into_iter().chain(
    entries
      .iter()
      .filter(|entry| delivering.is_none() && entry.state == QueueState::Queued),
  );
  for entry in candidates {
    let run = db.run(entry.run).map_err(run_store_error)?;
    let waits =
      entry.state == QueueState::Queued && run.as_ref().is_some_and(|run| *run.status() != RunStatus::Succeeded);
    if !waits {
      return Ok(Some((entry.clone(), run)));
    }
  }
  Ok(None)
}

struct Driver<'a> {
  db: ProjectDb,
  root: &'a Path,
  programs: &'a Programs,
}

impl Driver<'_> {
  fn step(self, entry: QueueEntry, run: Option<Run>) -> Result<Advance, StoreError> {
    let Some(run) = run else {
      return self.leave(entry, stopped("the run cannot be read"), Vec::new());
    };
    if !self.latest(&run)? {
      return self.leave(entry, Left::LaterRun, Vec::new());
    }
    let Some(deliver) = run.snapshot().body().deliver().cloned() else {
      return self.leave(entry, stopped("the run's pipeline has no Deliver step"), Vec::new());
    };
    match entry.state.clone() {
      QueueState::Queued => self.save(entry, QueueState::Syncing, &[]),
      QueueState::Syncing => self.sync(entry, &run, &deliver),
      QueueState::Rechecking { base, tip } => self.recheck(entry, &run, (base, tip), &deliver),
      QueueState::Integrating { after, files, .. } => self.integrating(entry, &run, (after, files)),
      QueueState::Blocked(_) => self.land(entry, &run, (&deliver, Vec::new())),
      QueueState::WaitingForApproval | QueueState::Left(_) | QueueState::Merged(_) | QueueState::Removed => {
        Ok(Advance::Idle { poll: false })
      }
    }
  }

  /// Whether `run` is its task's latest run.
  fn latest(&self, run: &Run) -> Result<bool, StoreError> {
    let runs = self.db.runs_of_task(run.task()).map_err(run_store_error)?;
    let number = run.number().get();
    Ok(!runs.iter().any(|listed| match listed {
      Ok(listed) => listed.number().get() > number,
      Err(unreadable) => unreadable.number.is_some_and(|other| other.get() > number),
    }))
  }

  fn sync(self, entry: QueueEntry, run: &Run, deliver: &DeliverStep) -> Result<Advance, StoreError> {
    let Some(base) = run.base_branch() else {
      return self.leave(entry, stopped("the run did not record its base branch"), Vec::new());
    };
    let worktree = run_worktree(self.root, run.id());
    let synced = delivery::sync(
      &self.programs.git_work(),
      Path::new(GIT),
      (&worktree, base, run.branch()),
    );
    match synced {
      Ok(Synced::Clean { base, tip }) => {
        let events = vec![QueueEvent::Synced {
          base: base.clone(),
          files: Vec::new(),
        }];
        let checked_already = entry
          .checked
          .as_ref()
          .is_some_and(|checked| checked.base == base && checked.tip == tip);
        if checked_already || !deliver.recheck() {
          return self.checked(entry, run, (deliver, events));
        }
        let mut events = events;
        events.push(QueueEvent::CheckStarted { base: base.clone() });
        self.save(entry, QueueState::Rechecking { base, tip }, &events)
      }
      Ok(Synced::Conflicts { base, files }) => self.conflicts(entry, run, deliver, (base, files)),
      Err(error) => self.leave(entry, stopped(&error.to_string()), Vec::new()),
    }
  }

  /// Sends `files`, which conflicted with the base at `base`, to the run's Integrate step, or to a person when it has
  /// none.
  fn conflicts(
    self,
    entry: QueueEntry,
    run: &Run,
    deliver: &DeliverStep,
    (base, files): (Revision, Vec<String>),
  ) -> Result<Advance, StoreError> {
    let events = vec![QueueEvent::Synced {
      base: base.clone(),
      files: files.clone(),
    }];
    let integrating = integrate_step(run, deliver).and_then(|step| integrate(run, step, (&base, files.clone())));
    let Some((command, state)) = integrating else {
      return self.leave(entry, Left::Conflicts { files }, events);
    };
    let run_id = entry.run;
    match self.save(entry, state, &events)? {
      Advance::Changed => Ok(Advance::Integrate { run: run_id, command }),
      other => Ok(other),
    }
  }

  fn recheck(
    self,
    mut entry: QueueEntry,
    run: &Run,
    (base, tip): (Revision, Revision),
    deliver: &DeliverStep,
  ) -> Result<Advance, StoreError> {
    let worktree = run_worktree(self.root, run.id());
    let tools = match check(&self.db, self.programs, (self.root, &worktree), run)? {
      Checked::Failed { tool, detail } => {
        let events = vec![QueueEvent::CheckFailed {
          base,
          tool: tool.clone(),
        }];
        return self.leave(entry, Left::CheckFailed { tool, detail }, events);
      }
      Checked::Passed { tools } => tools,
      Checked::NothingToRun => String::new(),
    };
    let mut events = vec![QueueEvent::CheckPassed {
      base: base.clone(),
      tool: tools,
    }];
    let base_now = run
      .base_branch()
      .map(|branch| branch_head(&self.programs.runner(), Path::new(GIT), (self.root, branch)))
      .transpose()
      .map_err(|error| StoreError::Corrupt {
        what: "base branch",
        detail: error.to_string(),
      })?
      .flatten();
    if let Some(now) = base_now.filter(|now| *now != base) {
      events.push(QueueEvent::CheckSuperseded { base, now });
      return self.superseded(entry, run, events);
    }
    entry.checked = Some(CheckedOn {
      base,
      tip,
      at: Timestamp::now(),
    });
    entry.resyncs = 0;
    self.checked(entry, run, (deliver, events))
  }

  /// The base moved while Check ran: the run syncs again, or leaves once it did so [`MAX_RESYNCS`] times in a row.
  fn superseded(self, mut entry: QueueEntry, run: &Run, events: Vec<QueueEvent>) -> Result<Advance, StoreError> {
    if entry.resyncs >= MAX_RESYNCS {
      let mut heads = superseded_heads(&self.db, run.id())?;
      heads.extend(events.iter().filter_map(|event| match event {
        QueueEvent::CheckSuperseded { now, .. } => Some(now.clone()),
        _ => None,
      }));
      let base = run
        .base_branch()
        .map_or_else(String::new, |branch| branch.as_str().to_owned());
      return self.leave(entry, Left::KeepsMoving { base, heads }, events);
    }
    entry.resyncs += 1;
    entry.checked = None;
    self.save(entry, QueueState::Syncing, &events)
  }

  /// Checked on the base as it is (or not checked again, as the pipeline says): it waits for a person's approval
  /// unless it has it or merges on its own.
  fn checked(
    self,
    entry: QueueEntry,
    run: &Run,
    (deliver, events): (&DeliverStep, Vec<QueueEvent>),
  ) -> Result<Advance, StoreError> {
    if deliver.mode() == DeliverMode::ApproveToMerge && entry.approved_at.is_none() {
      return self.save(entry, QueueState::WaitingForApproval, &events);
    }
    self.land(entry, run, (deliver, events))
  }

  fn land(
    self,
    entry: QueueEntry,
    run: &Run,
    (deliver, mut events): (&DeliverStep, Vec<QueueEvent>),
  ) -> Result<Advance, StoreError> {
    let Some(task) = self.db.task(run.task())? else {
      return self.leave(entry, stopped("the run's task cannot be read"), events);
    };
    match land(
      (self.root, self.programs),
      (run, &task),
      (deliver, entry.checked.as_ref()),
    ) {
      Landed::Merged(delivered) => {
        events.push(QueueEvent::Merged {
          revision: delivered.revision.clone(),
          kind: delivered.kind,
        });
        events.push(QueueEvent::CleanedUp {
          worktree_removed: delivered.worktree_removed,
          branch_deleted: delivered.branch_deleted,
        });
        self.save(entry, QueueState::Merged(delivered), &events)
      }
      Landed::Blocked(blocked) if entry.state == QueueState::Blocked(blocked.clone()) && events.is_empty() => {
        Ok(Advance::Idle { poll: true })
      }
      Landed::Blocked(blocked) => {
        events.push(QueueEvent::Blocked(blocked.clone()));
        self.save(entry, QueueState::Blocked(blocked), &events)
      }
      Landed::Moved => self.save(entry, QueueState::Syncing, &events),
      Landed::Stopped(detail) => self.leave(entry, Left::Stopped { detail }, events),
    }
  }

  /// The Integrate step works on `files`; the run's events since it went were `after`.
  fn integrating(
    self,
    entry: QueueEntry,
    run: &Run,
    (after, files): (u64, Vec<String>),
  ) -> Result<Advance, StoreError> {
    if run.last_sequence().get() <= after {
      return Ok(Advance::Idle { poll: false });
    }
    match run.status() {
      RunStatus::Succeeded => self.save(entry, QueueState::Syncing, &[]),
      RunStatus::Failed(_) | RunStatus::Cancelled => self.leave(entry, Left::Conflicts { files }, Vec::new()),
      _ => Ok(Advance::Idle { poll: false }),
    }
  }

  fn leave(self, entry: QueueEntry, left: Left, mut events: Vec<QueueEvent>) -> Result<Advance, StoreError> {
    events.push(QueueEvent::Left(left.clone()));
    self.save(entry, QueueState::Left(left), &events)
  }

  /// Saves `entry` in `state` with `events`, unless it moved on meanwhile: either way the queue changed.
  fn save(mut self, mut entry: QueueEntry, state: QueueState, events: &[QueueEvent]) -> Result<Advance, StoreError> {
    let expected = std::mem::replace(&mut entry.state, state);
    let now = Timestamp::now();
    if entry.state != expected {
      entry.since = now;
    }
    if !self.db.save_merge_queue_entry(&entry, &expected, (events, now))? {
      tracing::info!(run = %entry.run, "the merge queue entry changed meanwhile; it is read again");
    }
    Ok(Advance::Changed)
  }
}

fn stopped(detail: &str) -> Left {
  Left::Stopped {
    detail: detail.to_owned(),
  }
}

/// The heads the base moved to during the run's checks since it last passed one on the base as it was.
fn superseded_heads(db: &ProjectDb, run: RunId) -> Result<Vec<Revision>, StoreError> {
  let events = db.merge_queue_events(run)?;
  let mut heads = Vec::new();
  for recorded in events.iter().rev() {
    match &recorded.event {
      QueueEvent::CheckSuperseded { now, .. } => heads.push(now.clone()),
      QueueEvent::CheckPassed { .. } | QueueEvent::Synced { .. } | QueueEvent::CheckStarted { .. } => {}
      _ => break,
    }
  }
  heads.reverse();
  Ok(heads)
}

/// How sending `run`'s conflicts to its Integrate step went (`sent`: why not, when it did not): it works on them, or
/// the run leaves the queue and its conflicts wait for a person. Blocks on SQLite.
pub fn integrate_sent(root: &Path, run: RunId, sent: Result<(), Message>) -> Result<(), Message> {
  let mut db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let entry = db
    .merge_queue_entry(run)
    .map_err(|error| messages::store_error(&error))?;
  let Some(mut entry) = entry else {
    return Ok(());
  };
  let QueueState::Integrating { step, files, .. } = entry.state.clone() else {
    return Ok(());
  };
  let expected = entry.state.clone();
  let now = Timestamp::now();
  let events = match sent {
    Ok(()) => vec![QueueEvent::IntegrateStarted { step, files }],
    Err(message) => {
      tracing::warn!(key = message.key, %run, "the run's conflicts could not go to its Integrate step");
      let left = Left::Conflicts { files };
      entry.state = QueueState::Left(left.clone());
      entry.since = now;
      vec![QueueEvent::Left(left)]
    }
  };
  db.save_merge_queue_entry(&entry, &expected, (&events, now))
    .map_err(|error| messages::store_error(&error))?;
  wake(root);
  Ok(())
}
