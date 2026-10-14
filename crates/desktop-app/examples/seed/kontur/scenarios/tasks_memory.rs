//! `tasks-memory`: the `tasks` project with two records its agents wrote to Kontur's memory, which wait
//! for a person's approval (`Needs you · Memory` in `q9mf0k`): one by KON-12's Builder at Implement, one by
//! KON-16's Planner at Plan, as the design's "from the KON-12 and KON-16 handoffs".

use orchester_core::{
  memory::{
    AgentProvenance, AgentWrite, Confidence, Memory, MemoryContent, MemoryKind, MemoryScope, MemorySettings,
    MemoryStore, SecretScreen,
  },
  run::StepExecutor,
  store::RunStore,
};

use super::tasks;
use crate::kontur::{Kontur, SeedError};

/// One record: the task whose latest run wrote it, at which step, what it says and how many minutes ago.
struct Record {
  task: &'static str,
  step: &'static str,
  text: &'static str,
  minutes: i64,
}

const RECORDS: [Record; 2] = [
  Record {
    task: "KON-12",
    step: "implement",
    text: "Webhook deliveries retry with exponential backoff; the endpoint's delivery log keeps every attempt.",
    minutes: 50,
  },
  Record {
    task: "KON-16",
    step: "plan",
    text: "GitHub's secondary rate limits apply per installation, not per token.",
    minutes: 20,
  },
];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  tasks::project(kontur)?;
  let records = RECORDS
    .iter()
    .map(|record| memory(kontur, record))
    .collect::<Result<Vec<_>, _>>()?;
  kontur.db.add_memories(&records)?;
  Ok(())
}

/// `record`, written by the agent of its step in its task's latest run.
fn memory(kontur: &Kontur, record: &Record) -> Result<Memory, SeedError> {
  let key = record.task;
  let task = kontur.find(key)?;
  let run = kontur
    .db
    .runs_of_task(task.id())?
    .pop()
    .ok_or_else(|| SeedError::Invalid(format!("{key} has no run")))?
    .map_err(|unreadable| SeedError::Invalid(format!("{key}'s run cannot be read: {unreadable:?}")))?;
  let execution = run
    .current_attempt()
    .and_then(|attempt| {
      attempt
        .executions()
        .iter()
        .rfind(|execution| execution.step().as_str() == record.step)
    })
    .ok_or_else(|| SeedError::NoSuchStep(record.step.to_owned()))?;
  let profile = match run.snapshot().step(execution.step()).map(|step| step.executor()) {
    Some(StepExecutor::Agent { profile, .. }) => profile.resource,
    executor => {
      return Err(SeedError::Invalid(format!(
        "{key}'s step {} is no agent's: {executor:?}",
        execution.step()
      )));
    }
  };
  let write = AgentWrite {
    provenance: AgentProvenance {
      profile,
      run: run.id(),
      step: execution.step().clone(),
      execution: execution.id(),
    },
    confidence: Confidence::Medium,
    sources: Vec::new(),
  };
  let content =
    MemoryContent::new(MemoryKind::Fact, record.text).map_err(|error| SeedError::Invalid(error.to_string()))?;
  let screened = SecretScreen::new(Vec::new())
    .check(content)
    .map_err(|error| SeedError::Invalid(error.to_string()))?;
  // The default settings write an agent's record after approval: it waits for a person.
  Ok(Memory::by_agent(
    MemoryScope::Project,
    write,
    screened,
    &MemorySettings::default(),
    kontur.ago(record.minutes),
  ))
}
