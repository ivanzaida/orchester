//! What the line of an event of a task's run (ORC-755, [`orchester_core::orchestrator::TaskEventKind`]) names beside
//! its wait (`Parts · Lines` e `c9YRv` in `Orchestrator watch · redesign`): the module its task is in, the branch the
//! run merges into, the task the run waits on, by its key and its run's number, and for a stall that no longer stands
//! what became of its run (ORC-894). The wait keeps only ids of these, so they are read as the conversation is; and so
//! is how each wait a *waits on* row names stands now (ORC-784).

use std::collections::{HashMap, HashSet};

use orchester_core::{
  id::RunId,
  orchestrator::{ChatAction, ChatEntry, EntryKind, TaskEventKind},
  store::{RunStore, TaskStore},
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  chat::{EventFacts, WaitedTask},
  stalls,
  waits_on::view::{self, RunWaitView},
};
use crate::tasks::detail::run_store_error;

/// The facts of each event among `entries`, by entry, those whose seq is in `standing` still standing. What the
/// project no longer keeps is left out of them. Blocks on SQLite.
pub(super) fn event_facts(
  db: &ProjectDb,
  entries: &[ChatEntry],
  standing: &HashSet<i64>,
) -> Result<HashMap<i64, EventFacts>, StoreError> {
  let mut facts = HashMap::new();
  for entry in entries {
    let EntryKind::Wait { wait, .. } = &entry.kind else {
      continue;
    };
    let Some(event) = wait.kind.event() else {
      continue;
    };
    let run = db.run(wait.run).map_err(run_store_error)?;
    let task = match &run {
      Some(run) => db.task(run.task())?,
      None => None,
    };
    let module = match task.as_ref().and_then(|task| task.module()) {
      Some(module) => db.module(module)?.map(|module| module.name().as_str().to_owned()),
      None => None,
    };
    let branch = run
      .as_ref()
      .and_then(|run| run.base_branch())
      .map(|branch| branch.as_str().to_owned());
    let settled = match event.kind {
      TaskEventKind::NoProgress { since, .. } if !standing.contains(&entry.seq) => {
        Some(stalls::settled(db, run.as_ref(), since)?)
      }
      _ => None,
    };
    facts.insert(
      entry.seq,
      EventFacts {
        module,
        branch,
        waited: waited(db, event.kind)?,
        settled,
      },
    );
  }
  Ok(facts)
}

/// How the wait of each run a *waits on* row among `entries` names stands now (ORC-784), by the run. A run the project
/// no longer keeps is left out. Blocks on SQLite.
pub(super) fn waits_on(db: &ProjectDb, entries: &[ChatEntry]) -> Result<HashMap<RunId, RunWaitView>, StoreError> {
  let mut waits = HashMap::new();
  for entry in entries {
    let EntryKind::Action(ChatAction {
      waits_on: Some(row), ..
    }) = &entry.kind
    else {
      continue;
    };
    if waits.contains_key(&row.run) {
      continue;
    }
    let Some(run) = db.run(row.run).map_err(run_store_error)? else {
      continue;
    };
    if let Some(wait) = view::read(db, &run)? {
      waits.insert(row.run, wait);
    }
  }
  Ok(waits)
}

/// The task the run of an event of `kind` waits on, while the project keeps it.
fn waited(db: &ProjectDb, kind: TaskEventKind) -> Result<Option<WaitedTask>, StoreError> {
  let (task, run) = match kind {
    TaskEventKind::WaitedMerged { task, run } => (task, Some(run)),
    TaskEventKind::WaitedClosed { task, run } => (task, run),
    TaskEventKind::NoProgress { .. } | TaskEventKind::MergeConflict => return Ok(None),
  };
  let Some(task) = db.task(task)? else {
    return Ok(None);
  };
  let number = match run {
    Some(run) => db.run(run).map_err(run_store_error)?.map(|run| run.number().get()),
    None => None,
  };
  Ok(Some(WaitedTask {
    key: task.key().to_string(),
    number,
  }))
}
