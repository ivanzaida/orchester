//! What the next invocation of a step gets from memory. The runner calls it as an agent starts; a
//! task's Memory tab calls it to show what the next agent receives, so both see the same records.

use serde::{Deserialize, Serialize};

use super::{MemoryFilter, MemoryQuery, MemoryStatus, MemoryStore, RecallRequest, Recalled, ScopeFilter, recall};
use crate::{
  id::{ResourceId, TaskId},
  resource::Step,
  task::Task,
  time::Timestamp,
};

/// Matches fetched per record the prompt may get, so ranking has more than the best matches to
/// choose from.
const MATCHES_PER_RECORD: usize = 4;

/// The step an agent is about to work on, and who runs it.
#[derive(Clone, Copy, Debug)]
pub struct StepContext<'a> {
  pub task: &'a Task,
  pub step: &'a Step,
  /// The agent profile that runs the step.
  pub profile: ResourceId,
  /// Its role.
  pub role: &'a str,
}

impl StepContext<'_> {
  /// What the step's records are chosen by, as a plain value.
  pub fn recall(&self) -> StepRecall {
    let details = self.task.details();
    StepRecall {
      task: self.task.id(),
      title: details.title.as_str().to_owned(),
      brief: details.brief.clone(),
      step: self.step.name.as_str().to_owned(),
      instructions: self.step.instructions.as_str().to_owned(),
      profile: self.profile,
      role: self.role.to_owned(),
    }
  }
}

/// What the records of a step's next invocation are chosen by: its task, the texts searched with (the
/// task's title and brief, the step's name and instructions) and the agent profile that runs it, with
/// its role. A plain value, so whoever holds the memory (the coordinator of a run) can be asked for them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRecall {
  pub task: TaskId,
  pub title: String,
  pub brief: String,
  pub step: String,
  pub instructions: String,
  pub profile: ResourceId,
  pub role: String,
}

/// The records the next invocation of `context`'s step gets at `now` ([`step_memory`]).
pub fn next_invocation_memory<S: MemoryStore + ?Sized>(
  store: &S,
  context: StepContext<'_>,
  now: Timestamp,
) -> Result<Recalled, S::Error> {
  step_memory(store, &context.recall(), now)
}

/// The records the next invocation of `step` gets at `now`: the task's active records, the project's
/// pinned ones and those matching the step's name and instructions and the task's title and brief,
/// ranked and capped by the project's Memory setting as it is now.
pub fn step_memory<S: MemoryStore + ?Sized>(
  store: &S,
  step: &StepRecall,
  now: Timestamp,
) -> Result<Recalled, S::Error> {
  let settings = store.memory_settings()?;
  let task = step.task;
  let active = |scope| MemoryFilter {
    scope,
    status: Some(MemoryStatus::Active),
    pinned_only: false,
  };
  let mut candidates = store.memories(&active(ScopeFilter::Task(task)))?;
  let pinned = MemoryFilter {
    pinned_only: true,
    ..active(ScopeFilter::Project)
  };
  candidates.extend(store.memories(&pinned)?);
  let query = MemoryQuery::for_step(&step.title, &step.brief, &step.step, &step.instructions);
  let limit = usize::from(settings.limits.records()) * MATCHES_PER_RECORD;
  let matches = store.search_memories(task, &query, limit)?;
  let request = RecallRequest {
    task,
    profile: step.profile,
    role: &step.role,
    limits: settings.limits,
    now,
  };
  Ok(recall(&request, candidates, matches))
}
