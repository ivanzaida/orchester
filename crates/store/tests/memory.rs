//! Task and project memory in the project database: records and their changes, scope filters,
//! full-text search by word stems and trigram fragments, the Memory setting and what each
//! invocation was given. Run with `cargo test -p orchester-store --test memory memory`.

mod support;

use std::num::{NonZeroU16, NonZeroU32};

use orchester_core::{
  id::{IdempotencyKey, InvocationId, MemoryId, ResourceId, TaskId},
  memory::{
    AgentProvenance, InjectedMemory, InjectionReason, Memory, MemoryContent, MemoryFilter, MemoryKey, MemoryKind,
    MemoryLimits, MemoryQuery, MemoryScope, MemorySettings, MemoryStatus, MemoryStore, ScopeFilter, SecretScreen, Tags,
    WritePolicy,
  },
  resource::StepKey,
  store::{ActivityStore, TaskStore},
  task::TaskDetails,
};
use orchester_store::{ProjectDb, StoreError};
use support::{at, model, name, started_run};

fn content(kind: MemoryKind, text: &str) -> MemoryContent {
  MemoryContent::new(kind, text).expect("content")
}

fn person(scope: MemoryScope, content: MemoryContent, millis: i64) -> Memory {
  let screened = SecretScreen::new(Vec::new()).check(content).expect("no secrets");
  Memory::by_person(scope, screened, at(millis))
}

fn filter(scope: ScopeFilter) -> MemoryFilter {
  MemoryFilter {
    scope,
    status: None,
    pinned_only: false,
  }
}

fn texts(memories: &[Memory]) -> Vec<&str> {
  memories.iter().map(|memory| memory.content().text()).collect()
}

fn search(db: &ProjectDb, task: TaskId, text: &str) -> Vec<String> {
  db.search_memories(task, &MemoryQuery::from_text(text), 10)
    .expect("search")
    .into_iter()
    .map(|found| found.memory.content().text().to_owned())
    .collect()
}

/// A second task of the project.
fn another_task(db: &mut ProjectDb) -> TaskId {
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

#[test]
fn memory_records_are_stored_superseded_deprecated_and_deleted() {
  let (_repo, mut db, run, execution) = started_run();
  let task = run.task();
  let agent = AgentProvenance {
    profile: ResourceId::generate(),
    run: run.id(),
    step: StepKey::new("implement").expect("key"),
    execution,
  };
  let rich = content(MemoryKind::Decision, "Use SQLite.")
    .with_key(MemoryKey::new("db.engine").expect("key"))
    .with_tags(Tags::new(["storage", "db"]).expect("tags"));
  let screened = SecretScreen::new(Vec::new()).check(rich).expect("no secrets");
  let mut first = Memory::by_agent(
    MemoryScope::Task(task),
    agent,
    screened,
    vec![3, 4],
    &MemorySettings::default(),
    at(10),
  );
  db.add_memories(std::slice::from_ref(&first)).expect("add");
  assert_eq!(db.memory(first.id()).expect("read"), Some(first.clone()));

  let replacement = first
    .supersede(
      SecretScreen::new(Vec::new())
        .check(content(MemoryKind::Decision, "Use SQLite with FTS5."))
        .expect("no secrets"),
      at(20),
    )
    .expect("supersede");
  db.supersede_memory(&first, &replacement).expect("store both");
  assert_eq!(
    db.memory(first.id()).expect("read").map(|memory| memory.status()),
    Some(MemoryStatus::Superseded)
  );
  assert_eq!(db.memory(replacement.id()).expect("read"), Some(replacement.clone()));
  assert!(matches!(
    db.supersede_memory(&first, &replacement),
    Err(StoreError::MemoryNotActive(id)) if id == first.id()
  ));

  let mut current = replacement;
  current.deprecate(at(30)).expect("deprecate");
  db.save_memory(&current).expect("save");
  assert_eq!(db.memory(current.id()).expect("read"), Some(current.clone()));

  db.delete_memory(first.id()).expect("delete");
  assert_eq!(db.memory(first.id()).expect("read"), None);
  let orphan = db.memory(current.id()).expect("read").expect("still stored");
  assert_eq!(orphan.supersedes(), None, "the deleted record is no longer named");
  assert!(matches!(
    db.delete_memory(MemoryId::generate()),
    Err(StoreError::MemoryNotFound(_))
  ));
}

#[test]
fn memory_listings_filter_by_scope_status_and_pin() {
  let (_repo, mut db, run, _) = started_run();
  let task = run.task();
  let for_task = person(MemoryScope::Task(task), content(MemoryKind::Fact, "Task fact."), 1);
  let mut pinned = person(MemoryScope::Project, content(MemoryKind::Fact, "Pinned fact."), 2);
  pinned.set_pinned(true, at(3)).expect("pin");
  let mut old = person(MemoryScope::Project, content(MemoryKind::Fact, "Old fact."), 4);
  old.deprecate(at(5)).expect("deprecate");
  db.add_memories(&[for_task, pinned, old]).expect("add");

  assert_eq!(
    texts(&db.memories(&filter(ScopeFilter::Task(task))).expect("list")),
    ["Task fact."]
  );
  assert_eq!(
    texts(&db.memories(&filter(ScopeFilter::Project)).expect("list")),
    ["Pinned fact.", "Old fact."]
  );
  assert_eq!(
    texts(&db.memories(&filter(ScopeFilter::TaskAndProject(task))).expect("list")),
    ["Task fact.", "Pinned fact.", "Old fact."]
  );
  let active = MemoryFilter {
    status: Some(MemoryStatus::Active),
    ..filter(ScopeFilter::TaskAndProject(task))
  };
  assert_eq!(
    texts(&db.memories(&active).expect("list")),
    ["Task fact.", "Pinned fact."]
  );
  let pinned_only = MemoryFilter {
    pinned_only: true,
    ..active
  };
  assert_eq!(texts(&db.memories(&pinned_only).expect("list")), ["Pinned fact."]);
  assert!(
    db.memories(&filter(ScopeFilter::Task(TaskId::generate())))
      .expect("list")
      .is_empty()
  );
}

#[test]
fn memory_search_finds_word_forms_paths_and_identifiers() {
  let (_repo, mut db, run, _) = started_run();
  let task = run.task();
  let records = [
    person(
      MemoryScope::Task(task),
      content(
        MemoryKind::Decision,
        "Міграцію бази даних виконує runner перед стартом.",
      ),
      1,
    ),
    person(
      MemoryScope::Project,
      content(
        MemoryKind::Fact,
        "Step results are parsed in crates/core/src/run/result.rs.",
      ),
      2,
    ),
    person(
      MemoryScope::Project,
      content(MemoryKind::Warning, "Call parseStepResult only on the final message."),
      3,
    ),
    person(
      MemoryScope::Task(another_task(&mut db)),
      content(MemoryKind::Fact, "Міграції іншої задачі."),
      4,
    ),
  ];
  db.add_memories(&records).expect("add");

  assert_eq!(
    search(&db, task, "Коли запускати міграції бази?"),
    ["Міграцію бази даних виконує runner перед стартом."]
  );
  // `run` also finds `runner` as a word prefix; the path's own match ranks first.
  assert_eq!(
    search(&db, task, "Read run/result.rs first")[0],
    "Step results are parsed in crates/core/src/run/result.rs."
  );
  assert_eq!(
    search(&db, task, "Open result.rs"),
    ["Step results are parsed in crates/core/src/run/result.rs."]
  );
  assert_eq!(
    search(&db, task, "Where is StepResult read?"),
    ["Call parseStepResult only on the final message."]
  );
  assert!(search(&db, task, "unrelated").is_empty());

  let mut stale = records[1].clone();
  stale.deprecate(at(10)).expect("deprecate");
  db.save_memory(&stale).expect("save");
  assert!(
    search(&db, task, "result.rs").is_empty(),
    "only active records are found"
  );

  let mut edited = records[2].clone();
  edited
    .edit(
      SecretScreen::new(Vec::new())
        .check(content(MemoryKind::Warning, "Call readOutcome only once."))
        .expect("no secrets"),
      at(11),
    )
    .expect("edit");
  db.save_memory(&edited).expect("save");
  assert!(
    search(&db, task, "parseStepResult").is_empty(),
    "the index follows edits"
  );
  assert_eq!(search(&db, task, "readOutcome"), ["Call readOutcome only once."]);
}

#[test]
fn memory_settings_are_stored_per_project() {
  let (_repo, mut db, ..) = started_run();
  assert_eq!(db.memory_settings().expect("read"), MemorySettings::DEFAULT);
  let changed = MemorySettings {
    task_writes: WritePolicy::AfterApproval,
    project_writes: WritePolicy::Automatic,
    limits: MemoryLimits {
      records: NonZeroU16::new(4).expect("above zero"),
      tokens: NonZeroU32::new(1_200).expect("above zero"),
    },
  };
  db.save_memory_settings(&changed).expect("save");
  assert_eq!(db.memory_settings().expect("read"), changed);
}

#[test]
fn memory_injected_into_an_invocation_is_recorded_in_order() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let injected = [
    InjectedMemory {
      memory: MemoryId::generate(),
      reason: InjectionReason::Handoff,
      tokens: 120,
    },
    InjectedMemory {
      memory: MemoryId::generate(),
      reason: InjectionReason::Match,
      tokens: 8,
    },
  ];
  db.save_injected_memory(invocation.id, &injected).expect("save");
  assert_eq!(db.injected_memory(invocation.id).expect("read"), injected);
  assert!(
    db.save_injected_memory(invocation.id, &injected).is_err(),
    "written once"
  );
  assert!(matches!(
    db.save_injected_memory(InvocationId::generate(), &injected),
    Err(StoreError::InvocationNotFound(_))
  ));
  assert!(db.injected_memory(InvocationId::generate()).expect("read").is_empty());
}
