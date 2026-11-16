//! A checked snapshot waits for an actual SQLite writer, then refuses the draft it saved meanwhile. The mechanics are
//! shared by every kind that holds drafts; an instruction stands for them (a pipeline holds none, D-032).
use std::{cell::RefCell, collections::BTreeSet, sync::mpsc, thread, time::Duration};

use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    AgentTargets, Draft, DraftSaved, InjectionEvent, Instruction, InstructionScope, PromptText, ResourceBody, Slug,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

struct Wait {
  entered: mpsc::Sender<()>,
  committed: mpsc::Receiver<()>,
}
thread_local! {
  static WAIT: RefCell<Option<Wait>> = const { RefCell::new(None) };
}
fn wait_for_writer(_attempt: i32) -> bool {
  WAIT.with_borrow_mut(|wait| {
    let wait = wait.take().expect("one actual write-lock wait");
    wait.entered.send(()).expect("writer observes actual CAS contention");
    wait
      .committed
      .recv_timeout(Duration::from_secs(10))
      .expect("writer commits before CAS retries");
    true
  })
}

#[derive(Clone, Copy)]
enum Start {
  UnpublishedDraft,
  PublishedOnly,
  PublishedDraft,
}
fn instruction(text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("fixture text is valid"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("fixture instruction is valid")
}
fn project(root: &std::path::Path) -> ProjectDb {
  std::fs::create_dir(root.join(".git")).expect("private fixture repository is writable");
  ProjectDb::create(
    root,
    Project::new(
      ProjectId::generate(),
      ProjectKey::new("TEST").expect("fixture project key is valid"),
      Name::new("Race fixture").expect("fixture project name is valid"),
    ),
  )
  .expect("private fixture database is created")
}
fn setup(start: Start) -> (tempfile::TempDir, ProjectDb, ResourceId) {
  let folder = tempfile::tempdir().expect("private fixture directory is created");
  let mut db = project(folder.path());
  let original = instruction("Published snapshot.");
  let id = db
    .create_resource(
      Slug::new("rules").expect("fixture slug is valid"),
      Name::new("Before").expect("fixture resource name is valid"),
      &original,
      Timestamp::now(),
    )
    .expect("fixture instruction is stored")
    .id();
  if !matches!(start, Start::UnpublishedDraft) {
    db.publish_draft::<Instruction>(id, Timestamp::now())
      .expect("fixture version is published");
  }
  if matches!(start, Start::PublishedDraft) {
    db.save_draft(
      id,
      &Draft::new(
        instruction("Draft snapshot."),
        Some(orchester_core::resource::VersionNumber::FIRST),
        Timestamp::now(),
      ),
    )
    .expect("fixture versioned draft is stored");
  }
  (folder, db, id)
}
struct Worker {
  outcome: thread::JoinHandle<Result<DraftSaved, StoreError>>,
  ready: mpsc::Receiver<()>,
  start: mpsc::Sender<()>,
  entered: mpsc::Receiver<()>,
  committed: mpsc::Sender<()>,
}
fn worker(root: &std::path::Path, id: ResourceId) -> Worker {
  let root = root.to_path_buf();
  let (ready_tx, ready_rx) = mpsc::channel();
  let (start_tx, start_rx) = mpsc::channel();
  let (wait_tx, wait_rx) = mpsc::channel();
  let (commit_tx, commit_rx) = mpsc::channel();
  let worker = thread::spawn(move || {
    let mut checked = ProjectDb::open(&root).expect("worker opens the fixture database");
    let latest = checked
      .latest_version::<Instruction>(id)
      .expect("worker reads latest version");
    let draft = checked
      .draft::<Instruction>(id)
      .expect("worker reads the checked draft");
    let snapshot = draft
      .as_ref()
      .map(Draft::body)
      .or_else(|| latest.as_ref().map(|v| v.body()))
      .expect("fixture supplies a draft or published body")
      .clone();
    let based_on = draft
      .as_ref()
      .and_then(Draft::based_on)
      .or_else(|| latest.as_ref().map(|v| v.number()));
    WAIT.set(Some(Wait {
      entered: wait_tx,
      committed: commit_rx,
    }));
    checked
      .set_busy_handler_for_test(wait_for_writer)
      .expect("fixture connection accepts contention observer");
    ready_tx.send(()).expect("writer receives checked-snapshot signal");
    start_rx
      .recv_timeout(Duration::from_secs(10))
      .expect("writer reserves database before CAS starts");
    checked.save_draft_matching(
      id,
      &snapshot,
      latest.as_ref().map(|v| v.number()),
      &Draft::new(instruction("Stale save."), based_on, Timestamp::now()),
    )
  });
  Worker {
    outcome: worker,
    ready: ready_rx,
    start: start_tx,
    entered: wait_rx,
    committed: commit_tx,
  }
}
fn concurrent_draft(start: Start) {
  let (folder, db, id) = setup(start);
  let protected = instruction("Concurrent draft.");
  let worker = worker(folder.path(), id);
  worker
    .ready
    .recv_timeout(Duration::from_secs(10))
    .expect("worker has captured the old snapshot");
  let writer = rusqlite::Connection::open(folder.path().join(".orchester/orchester.db"))
    .expect("distinct writer opens the fixture database");
  writer
    .execute_batch("BEGIN IMMEDIATE")
    .expect("writer reserves the fixture database");
  worker.start.send(()).expect("worker starts the checked CAS attempt");
  worker
    .entered
    .recv_timeout(Duration::from_secs(10))
    .expect("checked CAS waits on actual SQLite writer");
  let based_on = if matches!(start, Start::UnpublishedDraft) {
    None
  } else {
    Some(1_i64)
  };
  writer.execute("INSERT INTO resource_drafts(resource_id,body,based_on,updated_at) VALUES(?1,?2,?3,?4) \
    ON CONFLICT(resource_id) DO UPDATE SET body=excluded.body,based_on=excluded.based_on,updated_at=excluded.updated_at",
    rusqlite::params![id.to_string(), serde_json::json!({"schema": Instruction::SCHEMA, "body": &protected}).to_string(),
      based_on, Timestamp::now().unix_millis()]).expect("writer saves its protected draft");
  writer
    .execute(
      "UPDATE resources SET name='Concurrent name' WHERE id=?1",
      [id.to_string()],
    )
    .expect("writer saves its distinctive resource name");
  writer
    .execute_batch("COMMIT")
    .expect("protected writer commits before CAS retries");
  worker
    .committed
    .send(())
    .expect("CAS worker is released after actual commit");
  assert!(
    matches!(worker.outcome.join().expect("CAS worker completes without panic"), Err(StoreError::DraftChanged(changed)) if changed == id)
  );
  let retained = db
    .draft::<Instruction>(id)
    .expect("retained draft can be read")
    .expect("protected draft still exists");
  assert_eq!(retained.body(), &protected);
  assert_eq!(retained.based_on().map(|v| i64::from(v.get())), based_on);
  assert_eq!(
    db.resource(id)
      .expect("retained resource can be read")
      .expect("fixture resource still exists")
      .name()
      .as_str(),
    "Concurrent name"
  );
}

#[test]
fn draft_matching_refuses_a_concurrent_draft_over_an_unpublished_snapshot() {
  concurrent_draft(Start::UnpublishedDraft);
}
#[test]
fn draft_matching_refuses_a_new_draft_after_a_no_draft_snapshot() {
  concurrent_draft(Start::PublishedOnly);
}
#[test]
fn draft_matching_refuses_a_concurrent_draft_over_an_existing_versioned_draft() {
  concurrent_draft(Start::PublishedDraft);
}
