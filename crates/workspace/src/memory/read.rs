//! Reading memory for its screens: a task's records with what its next agent gets (the task's Memory tab), the
//! project's records with every proposal waiting for approval (the project's Memory page), and the invocations
//! that were given one record (*Given to agents*, *Delete this record?*). Every function blocks on SQLite; the UI
//! calls them off the UI thread.

use std::path::Path;

use orchester_core::{
  id::{MemoryId, TaskId},
  memory::{GivenMemory, Memory, MemoryFilter, MemorySettings, MemoryStatus, MemoryStore, ScopeFilter},
  store::TaskStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  listing::Counts,
  next::{NextAgent, next_agent},
  origin::{Origins, Workplace},
};

/// A task's memory as its Memory tab shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskMemory {
  /// Every record of the task, whatever its status, oldest first.
  pub records: Vec<Memory>,
  pub origins: Origins,
  pub settings: MemorySettings,
  /// What the next agent of the task gets; `None` while no agent step works or waits to start.
  pub next: Option<NextAgent>,
  /// Records of the project that cannot be read back and are left out everywhere.
  pub unreadable: usize,
}

impl TaskMemory {
  pub fn load_in(repo_root: &Path, task: TaskId) -> Result<Self, StoreError> {
    let db = ProjectDb::open(repo_root)?;
    let read = db.task(task)?.ok_or(StoreError::TaskNotFound(task))?;
    let records = db.memories(&every(ScopeFilter::Task(task)))?;
    let next = next_agent(&db, &read, Timestamp::now())?;
    let mut named = records.clone();
    named.extend(
      next
        .iter()
        .flat_map(|next| next.given.iter().map(|given| given.memory.clone())),
    );
    Ok(Self {
      origins: Origins::load(&db, &named, [])?,
      records,
      settings: db.memory_settings()?,
      next,
      unreadable: db.unreadable_memories()?.len(),
    })
  }

  pub fn counts(&self) -> Counts {
    Counts::of(&self.records)
  }
}

/// How many active records task `task` has: the count beside its page's Memory tab.
pub fn active_count_in(repo_root: &Path, task: TaskId) -> Result<usize, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  let active = MemoryFilter {
    status: Some(MemoryStatus::Active),
    ..every(ScopeFilter::Task(task))
  };
  Ok(db.memories(&active)?.len())
}

/// The project's memory as its Memory page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectMemory {
  /// Every record of the project's own memory, whatever its status, oldest first.
  pub records: Vec<Memory>,
  /// Every proposal waiting for approval, the project's and every task's, oldest first.
  pub waiting: Vec<Memory>,
  /// The records the waiting proposals would replace, of whichever scope.
  pub replaced: Vec<Memory>,
  pub origins: Origins,
  pub settings: MemorySettings,
  pub unreadable: usize,
}

impl ProjectMemory {
  pub fn load_in(repo_root: &Path) -> Result<Self, StoreError> {
    let db = ProjectDb::open(repo_root)?;
    let records = db.memories(&every(ScopeFilter::Project))?;
    let waiting = db.memories(&MemoryFilter {
      status: Some(MemoryStatus::Proposed),
      ..every(ScopeFilter::All)
    })?;
    let mut replaced = Vec::new();
    for named in waiting.iter().filter_map(Memory::supersedes) {
      replaced.extend(db.memory(named)?);
    }
    let mut named = records.clone();
    named.extend(waiting.iter().cloned());
    named.extend(replaced.iter().cloned());
    Ok(Self {
      origins: Origins::load(&db, &named, [])?,
      records,
      waiting,
      replaced,
      settings: db.memory_settings()?,
      unreadable: db.unreadable_memories()?.len(),
    })
  }

  /// The project's records by status, with every proposal as waiting.
  pub fn counts(&self) -> Counts {
    Counts {
      waiting: self.waiting.len(),
      ..Counts::of(
        self
          .records
          .iter()
          .filter(|memory| memory.status() != MemoryStatus::Proposed),
      )
    }
  }

  /// The proposals that wait, of the project's own memory and of tasks'.
  pub fn waiting_by_scope(&self) -> (usize, usize) {
    let project = self
      .waiting
      .iter()
      .filter(|memory| memory.scope() == orchester_core::memory::MemoryScope::Project)
      .count();
    (project, self.waiting.len() - project)
  }
}

/// The invocations that were given one record, latest first, with where each worked.
#[derive(Clone, Debug, PartialEq)]
pub struct GivenTo {
  pub invocations: Vec<GivenMemory>,
  origins: Origins,
}

impl GivenTo {
  pub fn load_in(repo_root: &Path, memory: MemoryId) -> Result<Self, StoreError> {
    let db = ProjectDb::open(repo_root)?;
    let invocations = db.given_memory(memory)?;
    let origins = Origins::load(&db, &[], invocations.iter().map(|given| given.run))?;
    Ok(Self { invocations, origins })
  }

  /// Where `given`'s invocation worked.
  pub fn workplace(&self, given: &GivenMemory) -> Workplace {
    match self.origins.step_of(given.run, given.execution) {
      Some(step) => self.origins.at(given.run, &step.clone(), given.execution),
      None => Workplace::default(),
    }
  }
}

const fn every(scope: ScopeFilter) -> MemoryFilter {
  MemoryFilter {
    scope,
    status: None,
    pinned_only: false,
  }
}
