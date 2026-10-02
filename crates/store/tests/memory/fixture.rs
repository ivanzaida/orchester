//! Records, filters and the agent write of the memory tests.

#![allow(dead_code, reason = "shared by the memory test modules, each of which uses a part")]

pub use orchester_core::{
  id::{IdempotencyKey, InvocationId, MemoryId, ResourceId, StepExecutionId, TaskId},
  memory::{
    AgentProvenance, AgentWrite, Confidence, Delivery, InjectedMemory, InjectionReason, Memory, MemoryAuthor,
    MemoryChanges, MemoryContent, MemoryFilter, MemoryKey, MemoryKind, MemoryLimits, MemoryQuery, MemoryScope,
    MemorySettings, MemoryStatus, MemoryStore, ScopeFilter, ScreenedContent, SecretScreen, StepContext, Tags,
    WritePolicy, approval, next_invocation_memory,
  },
  resource::StepKey,
  run::Run,
  store::{ActivityStore, TaskStore},
  task::TaskDetails,
};
pub use orchester_store::{ProjectDb, StoreError};

pub use crate::support::{at, model, name, started_run};

pub fn content(kind: MemoryKind, text: &str) -> MemoryContent {
  MemoryContent::new(kind, text).expect("content")
}

pub fn person(scope: MemoryScope, content: MemoryContent, millis: i64) -> Memory {
  let screened = SecretScreen::new(Vec::new()).check(content).expect("no secrets");
  Memory::by_person(scope, screened, at(millis))
}

pub fn filter(scope: ScopeFilter) -> MemoryFilter {
  MemoryFilter {
    scope,
    status: None,
    pinned_only: false,
  }
}

pub fn texts(memories: &[Memory]) -> Vec<&str> {
  memories.iter().map(|memory| memory.content().text()).collect()
}

pub fn search(db: &ProjectDb, task: TaskId, text: &str) -> Vec<String> {
  db.search_memories(task, &MemoryQuery::from_text(text), 10)
    .expect("search")
    .into_iter()
    .map(|found| found.memory.content().text().to_owned())
    .collect()
}

/// A second task of the project.
pub fn another_task(db: &mut ProjectDb) -> TaskId {
  let details = TaskDetails {
    title: name("Another task"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: None,
  };
  db.create_tasks(IdempotencyKey::generate(), vec![details], at(1))
    .expect("task")[0]
    .id()
}

pub fn screened(content: MemoryContent) -> ScreenedContent {
  SecretScreen::new(Vec::new()).check(content).expect("no secrets")
}

pub fn agent_write(run: &Run, execution: StepExecutionId) -> AgentWrite {
  AgentWrite {
    provenance: AgentProvenance {
      profile: ResourceId::generate(),
      run: run.id(),
      step: StepKey::new("implement").expect("key"),
      execution,
    },
    confidence: Confidence::Medium,
    sources: vec![3, 4],
  }
}

pub fn keyed(text: &str) -> MemoryContent {
  content(MemoryKind::Decision, text).with_key(MemoryKey::new("db.engine").expect("key"))
}
