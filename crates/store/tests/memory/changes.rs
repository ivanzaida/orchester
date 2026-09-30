//! Deleting proposals, withdrawn proposals, stale copies and approvals in one transaction.

use super::fixture::*;

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
  db.save_memory(&mut rejected).expect("save");
  db.delete_memory(rejected.id()).expect("delete the rejected proposal");
  assert!(
    db.memory(active.id()).expect("read").is_some(),
    "the active record stays"
  );
}

#[test]
fn memory_deleting_a_withdrawn_proposal_keeps_every_history() {
  let (_repo, mut db, run, execution) = started_run();
  let settings = MemorySettings::default();
  let mut first = person(MemoryScope::Project, content(MemoryKind::Fact, "Use Postgres."), 1);
  db.add_memories(std::slice::from_ref(&first)).expect("add");
  let replacing = |text: &str, millis| {
    Memory::replacing(
      &first,
      agent_write(&run, execution),
      screened(content(MemoryKind::Fact, text)),
      &settings,
      at(millis),
    )
    .expect("a replacement")
  };
  let mut withdrawn = replacing("Use MySQL.", 2);
  let mut approved = replacing("Use SQLite.", 3);
  withdrawn.withdraw(at(3)).expect("withdraw");
  db.add_memories(std::slice::from_ref(&approved))
    .expect("add the newer proposal");
  db.add_memories(std::slice::from_ref(&withdrawn))
    .expect("add the withdrawn one");
  approved.approve(Some(&mut first), at(4)).expect("approve");
  db.supersede_memory(&mut first, &mut approved).expect("store both");
  assert_eq!(withdrawn.earlier_version(), None, "a withdrawn proposal is no version");
  assert_eq!(approved.earlier_version(), Some(first.id()));

  db.delete_memory(withdrawn.id()).expect("delete the withdrawn proposal");
  assert!(
    db.memory(first.id()).expect("read").is_some(),
    "the approved record's history stays"
  );
  assert!(db.memory(approved.id()).expect("read").is_some());
}

#[test]
fn memory_a_stale_copy_never_overwrites_a_newer_change() {
  let (_repo, mut db, ..) = started_run();
  let record = person(MemoryScope::Project, content(MemoryKind::Fact, "Stored."), 1);
  db.add_memories(std::slice::from_ref(&record)).expect("add");
  let mut first = db.memory(record.id()).expect("read").expect("stored");
  let mut stale = first.clone();
  first.deprecate(at(2)).expect("deprecate");
  db.save_memory(&mut first).expect("save");
  stale.set_pinned(true, at(3)).expect("pin the stale copy");
  assert!(matches!(
    db.save_memory(&mut stale),
    Err(StoreError::MemoryConflict(id)) if id == record.id()
  ));
  let stored = db.memory(record.id()).expect("read").expect("stored");
  assert_eq!((stored.status(), stored.is_pinned()), (MemoryStatus::Deprecated, false));
  assert_eq!(stored.revision(), first.revision());

  // A change made from what one transaction read; its own error stores nothing.
  let filter = filter(ScopeFilter::Project);
  let refused: Result<(), &str> = db
    .change_memories(&filter, |mut memories| {
      memories[0].restore(at(4)).map_err(|_| "cannot restore")?;
      Err("changed my mind")
    })
    .expect("the store works");
  assert_eq!(refused, Err("changed my mind"));
  db.change_memories(&filter, |mut memories| {
    memories[0].restore(at(5)).map_err(|_| "cannot restore")?;
    Ok::<_, &str>(MemoryChanges {
      added: Vec::new(),
      changed: memories,
    })
  })
  .expect("the store works")
  .expect("restored");
  let stored = db.memory(record.id()).expect("read").expect("stored");
  assert_eq!(stored.status(), MemoryStatus::Active);
}

#[test]
fn memory_approving_a_replacement_changes_every_record_in_one_transaction() {
  let (_repo, mut db, run, execution) = started_run();
  let mut old = person(MemoryScope::Project, keyed("Use Postgres."), 1);
  old.set_pinned(true, at(2)).expect("pin");
  db.add_memories(std::slice::from_ref(&old)).expect("add");
  let proposal = Memory::replacing(
    &old,
    agent_write(&run, execution),
    screened(keyed("Use SQLite.")),
    &MemorySettings::default(),
    at(3),
  )
  .expect("a replacement");
  db.add_memories(std::slice::from_ref(&proposal)).expect("add");
  db.change_memories(&filter(ScopeFilter::Project), |records| {
    approval(proposal.id(), records, at(4))
  })
  .expect("the store works")
  .expect("approved");
  let status = |id| {
    db.memory(id)
      .expect("read")
      .map(|memory| (memory.status(), memory.is_pinned()))
  };
  assert_eq!(status(old.id()), Some((MemoryStatus::Superseded, false)));
  assert_eq!(status(proposal.id()), Some((MemoryStatus::Active, true)));
}
