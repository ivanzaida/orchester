//! Full-text search, the Memory setting, what invocations were given and unreadable rows.

use super::fixture::*;

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
  db.save_memory(&mut stale).expect("save");
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
  db.save_memory(&mut edited).expect("save");
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
      delivery: Delivery::Truncated,
    },
    InjectedMemory {
      memory: MemoryId::generate(),
      reason: InjectionReason::Match,
      tokens: 8,
      delivery: Delivery::OverTokenLimit,
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
fn memory_unreadable_rows_are_listed_and_left_out_of_listings_and_search() {
  let (repo, mut db, run, _) = started_run();
  let task = run.task();
  let good = person(MemoryScope::Task(task), content(MemoryKind::Fact, "Readable wave."), 1);
  let bad = person(MemoryScope::Task(task), content(MemoryKind::Fact, "Damaged wave."), 2);
  db.add_memories(&[good, bad.clone()]).expect("add");
  let raw = rusqlite::Connection::open(repo.path().join(".orchester").join("orchester.db")).expect("open");
  raw
    .execute(
      "UPDATE memories SET key = 'two' || char(10) || 'lines' WHERE id = ?1",
      [bad.id().to_string()],
    )
    .expect("damage the row");
  assert_eq!(
    texts(&db.memories(&filter(ScopeFilter::Task(task))).expect("list")),
    ["Readable wave."]
  );
  assert_eq!(search(&db, task, "wave"), ["Readable wave."]);
  let unreadable = db.unreadable_memories().expect("listed");
  assert_eq!(unreadable.len(), 1);
  assert_eq!(unreadable[0].id, bad.id().to_string());
}
