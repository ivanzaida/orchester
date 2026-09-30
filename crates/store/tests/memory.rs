//! Task and project memory in the project database: records and their changes, scope filters,
//! full-text search by word stems and trigram fragments, the Memory setting and what each
//! invocation was given. Run with `cargo test -p orchester-store --test memory memory`.

mod support;

use orchester_core::{
  id::{IdempotencyKey, InvocationId, MemoryId, ResourceId, StepExecutionId, TaskId},
  memory::{
    AgentProvenance, AgentWrite, Confidence, InjectedMemory, InjectionReason, Memory, MemoryAuthor, MemoryContent,
    MemoryFilter, MemoryKey, MemoryKind, MemoryLimits, MemoryQuery, MemoryScope, MemorySettings, MemoryStatus,
    MemoryStore, ScopeFilter, ScreenedContent, SecretScreen, StepContext, Tags, WritePolicy, next_invocation_memory,
  },
  resource::StepKey,
  run::Run,
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

fn screened(content: MemoryContent) -> ScreenedContent {
  SecretScreen::new(Vec::new()).check(content).expect("no secrets")
}

fn agent_write(run: &Run, execution: StepExecutionId) -> AgentWrite {
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

fn keyed(text: &str) -> MemoryContent {
  content(MemoryKind::Decision, text).with_key(MemoryKey::new("db.engine").expect("key"))
}

#[test]
fn memory_records_are_stored_superseded_edited_deprecated_and_deleted_with_their_history() {
  let (_repo, mut db, run, execution) = started_run();
  let scope = MemoryScope::Task(run.task());
  let rich = keyed("Use SQLite.").with_tags(Tags::new(["storage", "db"]).expect("tags"));
  let settings = MemorySettings::default();
  let mut first = Memory::by_agent(scope, agent_write(&run, execution), screened(rich), &settings, at(10));
  db.add_memories(std::slice::from_ref(&first)).expect("add");
  assert_eq!(db.memory(first.id()).expect("read"), Some(first.clone()));

  // Task memory is saved automatically: the replacement is active at once.
  let mut replacement = Memory::replacing(
    &first,
    agent_write(&run, execution),
    screened(keyed("Use SQLite with FTS5.")),
    &settings,
    at(20),
  )
  .expect("a replacement");
  first.supersede(&mut replacement, at(20)).expect("supersede");
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
  current
    .edit(screened(keyed("Use SQLite with FTS5 and trigrams.")), at(25))
    .expect("edit");
  db.save_memory(&current).expect("save");
  let stored = db.memory(current.id()).expect("read").expect("stored");
  assert_eq!(stored.edited_at(), Some(at(25)));
  assert!(matches!(stored.author(), MemoryAuthor::Agent(_)), "the author stays");
  current.deprecate(at(30)).expect("deprecate");
  db.save_memory(&current).expect("save");
  assert_eq!(db.memory(current.id()).expect("read"), Some(current.clone()));

  db.delete_memory(current.id()).expect("delete");
  assert_eq!(db.memory(current.id()).expect("read"), None);
  assert_eq!(db.memory(first.id()).expect("read"), None, "its history goes with it");
  assert!(matches!(
    db.delete_memory(MemoryId::generate()),
    Err(StoreError::MemoryNotFound(_))
  ));
}

#[test]
fn memory_an_approved_replacement_supersedes_the_record_it_names() {
  let (_repo, mut db, run, execution) = started_run();
  let mut old = person(MemoryScope::Project, keyed("Use Postgres."), 1);
  old.set_pinned(true, at(2)).expect("pin");
  db.add_memories(std::slice::from_ref(&old)).expect("add");
  let mut proposal = Memory::replacing(
    &old,
    agent_write(&run, execution),
    screened(keyed("Use SQLite.")),
    &MemorySettings::default(),
    at(3),
  )
  .expect("a replacement");
  assert_eq!(proposal.status(), MemoryStatus::Proposed);
  db.add_memories(std::slice::from_ref(&proposal))
    .expect("a proposal may share the key");
  let mut alone = proposal.clone();
  alone.approve(None, at(4)).expect("approve alone");
  assert!(matches!(
    db.save_memory(&alone),
    Err(StoreError::DuplicateMemoryKey(key)) if key == "db.engine"
  ));
  proposal.approve(Some(&mut old), at(4)).expect("approve and supersede");
  db.supersede_memory(&old, &proposal).expect("store both");
  let stored = db.memory(proposal.id()).expect("read").expect("stored");
  assert_eq!((stored.status(), stored.is_pinned()), (MemoryStatus::Active, true));
  assert_eq!(
    db.memory(old.id()).expect("read").map(|memory| memory.status()),
    Some(MemoryStatus::Superseded)
  );
}

#[test]
fn memory_records_are_added_once() {
  let (_repo, mut db, ..) = started_run();
  let record = person(MemoryScope::Project, content(MemoryKind::Fact, "Once."), 1);
  db.add_memories(std::slice::from_ref(&record)).expect("add");
  let mut changed = record.clone();
  changed.set_pinned(true, at(2)).expect("pin");
  assert!(matches!(
    db.add_memories(&[changed]),
    Err(StoreError::MemoryExists(id)) if id == record.id()
  ));
  assert_eq!(db.memory(record.id()).expect("read"), Some(record));
}

#[test]
fn memory_a_key_names_one_active_record_of_its_scope() {
  let (_repo, mut db, run, _) = started_run();
  let task = MemoryScope::Task(run.task());
  db.add_memories(&[person(task, keyed("Use SQLite."), 1)]).expect("add");
  assert!(matches!(
    db.add_memories(&[person(task, keyed("Use Postgres."), 2)]),
    Err(StoreError::DuplicateMemoryKey(_))
  ));
  db.add_memories(&[person(MemoryScope::Project, keyed("Use Postgres."), 3)])
    .expect("another scope may use the key");
  let mut deprecated = person(MemoryScope::Project, keyed("Use MySQL."), 5);
  deprecated.deprecate(at(6)).expect("deprecate");
  db.add_memories(&[deprecated.clone()])
    .expect("a deprecated record may share the key");
  deprecated.restore(at(7)).expect("restore");
  assert!(matches!(
    db.save_memory(&deprecated),
    Err(StoreError::DuplicateMemoryKey(_))
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
    limits: MemoryLimits::new(4, 1_200).expect("valid limits"),
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
  assert_eq!(
    db.memory(injected[0].memory).expect("read"),
    None,
    "a record that is gone resolves to none"
  );
}

#[test]
fn memory_the_next_invocation_gets_what_recall_chooses_within_the_setting() {
  let (_repo, mut db, run, _) = started_run();
  let task = db.task(run.task()).expect("read").expect("task");
  let step = run
    .snapshot()
    .body()
    .step(&StepKey::new("implement").expect("key"))
    .expect("step")
    .clone();
  let scope = MemoryScope::Task(task.id());
  let decision = person(scope, content(MemoryKind::Decision, "Greet in English."), 1);
  let hello = person(
    MemoryScope::Project,
    content(MemoryKind::Fact, "Say hello with a wave."),
    2,
  );
  let unrelated = person(
    MemoryScope::Project,
    content(MemoryKind::Fact, "Deploys run on Fridays."),
    3,
  );
  db.add_memories(&[decision, hello, unrelated]).expect("add");
  let context = StepContext {
    task: &task,
    step: &step,
    profile: ResourceId::generate(),
    role: "Builder",
  };
  let next = next_invocation_memory(&db, context, at(10)).expect("recall");
  let given: Vec<(InjectionReason, &str)> = next
    .entries()
    .iter()
    .map(|entry| (entry.reason, entry.memory.content().text()))
    .collect();
  assert_eq!(
    given,
    [
      (InjectionReason::Decision, "Greet in English."),
      (InjectionReason::Match, "Say hello with a wave.")
    ]
  );
  assert_eq!(
    next.tokens(),
    next.injected().iter().map(|entry| entry.tokens).sum::<u32>()
  );

  let one = MemorySettings {
    limits: MemoryLimits::new(1, 500).expect("valid limits"),
    ..MemorySettings::DEFAULT
  };
  db.save_memory_settings(&one).expect("save");
  assert_eq!(
    next_invocation_memory(&db, context, at(10))
      .expect("recall")
      .entries()
      .len(),
    1
  );
}

#[test]
fn memory_deleting_a_proposal_keeps_the_record_it_would_replace() {
  let (_repo, mut db, run, execution) = started_run();
  let active = person(MemoryScope::Project, content(MemoryKind::Fact, "Use Postgres."), 1);
  db.add_memories(std::slice::from_ref(&active)).expect("add");
  let replacing = |millis| {
    Memory::replacing(
      &active,
      agent_write(&run, execution),
      screened(content(MemoryKind::Fact, "Use SQLite.")),
      &MemorySettings::default(),
      at(millis),
    )
    .expect("a replacement")
  };
  let proposal = replacing(2);
  db.add_memories(std::slice::from_ref(&proposal)).expect("add");
  db.delete_memory(proposal.id()).expect("delete the proposal");
  assert!(
    db.memory(active.id()).expect("read").is_some(),
    "the active record stays"
  );

  let mut rejected = replacing(3);
  db.add_memories(std::slice::from_ref(&rejected)).expect("add");
  rejected.reject(at(4)).expect("reject");
  db.save_memory(&rejected).expect("save");
  db.delete_memory(rejected.id()).expect("delete the rejected proposal");
  assert!(
    db.memory(active.id()).expect("read").is_some(),
    "the active record stays"
  );
}
