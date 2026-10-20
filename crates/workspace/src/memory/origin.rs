//! Who wrote a record and where, as a record's meta line names it (*Builder · KON-12 · Run 128 · 03 Implement ·
//! attempt 2*), and where an invocation that was given a record worked: the runs, tasks and agent profiles the
//! records name, read once for a screen. What no longer exists, or a run that cannot be read back, leaves its
//! part out; it never fails the screen.

use std::{
  collections::{HashMap, hash_map::Entry},
  num::NonZeroU32,
  sync::Arc,
};

use orchester_core::{
  id::{ResourceId, RunId, StepExecutionId, TaskId},
  memory::{Memory, MemoryAuthor, MemoryScope},
  project::TaskKey,
  resource::{AgentProfile, StepKey, VersionNumber},
  run::{Run, RunNumber, StepExecutor},
  store::{ResourceStore, RunStore, TaskStore},
};
use orchester_store::{ProjectDb, RunStoreError, StoreError};

/// Where an agent worked: each part is `None` once what it names is gone or cannot be read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Workplace {
  /// The role of the agent profile version that ran the step (*Builder*).
  pub agent: Option<Arc<str>>,
  pub task: Option<TaskKey>,
  pub run: Option<RunNumber>,
  /// The step's position in its run's pipeline, from 1, and its name.
  pub step: Option<(usize, Arc<str>)>,
  /// The attempt of the step's execution, when its run executed the step in more than one attempt.
  pub attempt: Option<NonZeroU32>,
}

/// The runs, tasks and agent roles a screen's records name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Origins {
  runs: HashMap<RunId, Run>,
  tasks: HashMap<TaskId, TaskKey>,
  roles: HashMap<(ResourceId, VersionNumber), Arc<str>>,
}

impl Origins {
  /// What `memories` and the runs `runs` name, read from `db`.
  pub fn load(db: &ProjectDb, memories: &[Memory], runs: impl IntoIterator<Item = RunId>) -> Result<Self, StoreError> {
    let mut origins = Self::default();
    let written = memories.iter().filter_map(|memory| match memory.author() {
      MemoryAuthor::Agent(agent) => Some(agent.run),
      MemoryAuthor::Person => None,
    });
    for run in written.chain(runs) {
      origins.add_run(db, run)?;
    }
    for memory in memories {
      if let MemoryScope::Task(task) = memory.scope() {
        origins.add_task(db, task)?;
      }
    }
    Ok(origins)
  }

  fn add_run(&mut self, db: &ProjectDb, id: RunId) -> Result<(), StoreError> {
    if self.runs.contains_key(&id) {
      return Ok(());
    }
    let run = match db.run(id) {
      Ok(Some(run)) => run,
      Ok(None) => return Ok(()),
      Err(RunStoreError::Store(error)) => return Err(error),
      Err(error) => {
        tracing::warn!(run = %id, %error, "a run that memory names cannot be read");
        return Ok(());
      }
    };
    self.add_task(db, run.task())?;
    for step in run.snapshot().steps() {
      if let StepExecutor::Agent { profile, .. } = step.executor()
        && let Entry::Vacant(entry) = self.roles.entry((profile.resource, profile.number))
        && let Some(version) = db.version::<AgentProfile>(profile.resource, profile.number)?
      {
        entry.insert(version.body().role().as_str().into());
      }
    }
    self.runs.insert(id, run);
    Ok(())
  }

  fn add_task(&mut self, db: &ProjectDb, id: TaskId) -> Result<(), StoreError> {
    if self.tasks.contains_key(&id) {
      return Ok(());
    }
    if let Some(task) = db.task(id)? {
      self.tasks.insert(id, task.key().clone());
    }
    Ok(())
  }

  /// The key of task `task`, while it exists.
  pub fn task_key(&self, task: TaskId) -> Option<&TaskKey> {
    self.tasks.get(&task)
  }

  /// Where the agent that wrote `memory` worked; `None` for a person's record.
  pub fn written(&self, memory: &Memory) -> Option<Workplace> {
    let MemoryAuthor::Agent(agent) = memory.author() else {
      return None;
    };
    Some(self.at(agent.run, &agent.step, agent.execution))
  }

  /// Where execution `execution` of step `step` of run `run` worked.
  pub fn at(&self, run: RunId, step: &StepKey, execution: StepExecutionId) -> Workplace {
    let Some(read) = self.runs.get(&run) else {
      return Workplace::default();
    };
    let steps = read.snapshot().body().steps();
    let numbered = steps
      .iter()
      .position(|candidate| candidate.key == *step)
      .map(|index| (index + 1, Arc::from(steps[index].name.as_str())));
    let agent = match read.snapshot().step(step).map(|snapshot| snapshot.executor()) {
      Some(StepExecutor::Agent { profile, .. }) => self.roles.get(&(profile.resource, profile.number)).cloned(),
      _ => None,
    };
    // An attempt is named only for a step its run executed more than once.
    let executed = read
      .attempts()
      .iter()
      .filter(|attempt| attempt.executions().iter().any(|each| each.step() == step))
      .count();
    let attempt = (executed > 1)
      .then(|| {
        read
          .attempts()
          .iter()
          .find(|attempt| attempt.execution(execution).is_some())
          .map(orchester_core::run::Attempt::number)
      })
      .flatten();
    Workplace {
      agent,
      task: self.tasks.get(&read.task()).cloned(),
      run: Some(read.number()),
      step: numbered,
      attempt,
    }
  }

  /// The step `execution` of run `run` executed, while the run can be read.
  pub fn step_of(&self, run: RunId, execution: StepExecutionId) -> Option<&StepKey> {
    self
      .runs
      .get(&run)
      .and_then(|read| read.execution(execution))
      .map(|executed| executed.step())
  }
}
