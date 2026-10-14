//! Records stored, superseded, edited, deprecated and deleted; keys, listings and scope filters.

use super::fixture::*;

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
  db.supersede_memory(&mut first, &mut replacement).expect("store both");
  assert_eq!(
    db.memory(first.id()).expect("read").map(|memory| memory.status()),
    Some(MemoryStatus::Superseded)
  );
  assert_eq!(db.memory(replacement.id()).expect("read"), Some(replacement.clone()));
  assert!(matches!(
    db.supersede_memory(&mut first, &mut replacement),
    Err(StoreError::MemoryNotActive(id)) if id == first.id()
  ));

  let mut current = replacement;
  current
    .edit(screened(keyed("Use SQLite with FTS5 and trigrams.")), at(25))
    .expect("edit");
  db.save_memory(&mut current).expect("save");
  let stored = db.memory(current.id()).expect("read").expect("stored");
  assert_eq!(stored.edited_at(), Some(at(25)));
  assert!(matches!(stored.author(), MemoryAuthor::Agent(_)), "the author stays");
  current.deprecate(at(30)).expect("deprecate");
  db.save_memory(&mut current).expect("save");
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
    db.save_memory(&mut alone),
    Err(StoreError::DuplicateMemoryKey(key)) if key == "db.engine"
  ));
  proposal.approve(Some(&mut old), at(4)).expect("approve and supersede");
  db.supersede_memory(&mut old, &mut proposal).expect("store both");
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
    db.save_memory(&mut deprecated),
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

  // One query for every scope by status: the project's and the task's records, oldest first.
  assert_eq!(
    texts(&db.memories_with_status(MemoryStatus::Active).expect("list")),
    ["Task fact.", "Pinned fact."]
  );
  assert_eq!(
    texts(&db.memories_with_status(MemoryStatus::Deprecated).expect("list")),
    ["Old fact."]
  );
  assert!(
    db.memories_with_status(MemoryStatus::Proposed)
      .expect("list")
      .is_empty()
  );
}
