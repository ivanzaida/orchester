//! What the next invocation of a step gets from memory. The runner calls it as an agent starts; a
//! task's Memory tab calls it to show what the next agent receives, so both see the same records.

use super::{MemoryFilter, MemoryQuery, MemoryStatus, MemoryStore, RecallRequest, Recalled, ScopeFilter, recall};
use crate::{id::ResourceId, resource::Step, task::Task, time::Timestamp};

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

/// The records the next invocation of `context`'s step gets at `now`: the task's active records,
/// the project's pinned ones and those matching the step's name and instructions and the task's
/// title and brief, ranked and capped by the project's Memory setting as it is now.
pub fn next_invocation_memory<S: MemoryStore + ?Sized>(
  store: &S,
  context: StepContext<'_>,
  now: Timestamp,
) -> Result<Recalled, S::Error> {
  let settings = store.memory_settings()?;
  let task = context.task.id();
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
  let details = context.task.details();
  let query = MemoryQuery::for_step(
    details.title.as_str(),
    &details.brief,
    context.step.name.as_str(),
    context.step.instructions.as_str(),
  );
  let limit = usize::from(settings.limits.records()) * MATCHES_PER_RECORD;
  let matches = store.search_memories(task, &query, limit)?;
  let request = RecallRequest {
    task,
    profile: context.profile,
    role: context.role,
    limits: settings.limits,
    now,
  };
  Ok(recall(&request, candidates, matches))
}
