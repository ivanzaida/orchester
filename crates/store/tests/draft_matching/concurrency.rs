//! A checked ungated snapshot waits for an actual SQLite writer, then refuses its newer gates.
use std::{cell::RefCell, sync::mpsc, thread, time::Duration};

use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{Draft, DraftSaved, Pipeline, PipelineEdit, RepositoryAccess, ResourceBody, Slug, StepKind},
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
fn original() -> Pipeline {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty fixture pipeline is valid"));
  let key = edit.add_step(Name::new("Build").expect("fixture step name is valid"), None);
  edit.update(&key, |step| {
    if let StepKind::Agent { profile, .. } = &mut step.kind {
      *profile = Some(ResourceId::generate());
    }
  });
  edit
    .validate()
    .expect("single fixture step is valid")
    .with_description("published snapshot".into())
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
fn gated(body: &Pipeline, tool_calls: bool) -> Pipeline {
  let mut edit = PipelineEdit::new(body.clone());
  let key = edit.steps()[0].key.clone();
  edit.update(&key, |step| {
    if tool_calls {
      let StepKind::Agent {
        permissions,
        repository,
        ..
      } = &mut step.kind
      else {
        panic!("agent fixture");
      };
      *repository = RepositoryAccess::ReadOnly;
      permissions.interactive_tool_approvals = true;
    } else {
      step.approval_gate = true;
    }
  });
  edit
    .validate()
    .expect("adding a gate preserves fixture pipeline validity")
    .with_description("concurrent protected draft".into())
}
fn setup(start: Start) -> (tempfile::TempDir, ProjectDb, ResourceId, Pipeline) {
  let folder = tempfile::tempdir().expect("private fixture directory is created");
  let mut db = project(folder.path());
  let original = original();
  let id = db
    .create_resource(
      Slug::new("pipeline").expect("fixture pipeline slug is valid"),
      Name::new("Before").expect("fixture resource name is valid"),
      &original,
      Timestamp::now(),
    )
    .expect("fixture pipeline is stored")
    .id();
  if !matches!(start, Start::UnpublishedDraft) {
    db.publish_draft::<Pipeline>(id, Timestamp::now())
      .expect("fixture version is published");
  }
  if matches!(start, Start::PublishedDraft) {
    db.save_draft(
      id,
      &Draft::new(
        original.with_description("draft snapshot".into()),
        Some(orchester_core::resource::VersionNumber::FIRST),
        Timestamp::now(),
      ),
    )
    .expect("fixture versioned draft is stored");
  }
  let expected = db
    .draft::<Pipeline>(id)
    .expect("fixture draft can be read")
    .map(|d| d.body().clone())
    .unwrap_or_else(|| {
      db.latest_version::<Pipeline>(id)
        .expect("fixture latest version can be read")
        .expect("published fixture has a latest version")
        .body()
        .clone()
    });
  (folder, db, id, expected)
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
      .latest_version::<Pipeline>(id)
      .expect("worker reads latest version");
    let draft = checked.draft::<Pipeline>(id).expect("worker reads the checked draft");
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
      &Draft::new(
        snapshot.clone().with_description("stale ungated save".into()),
        based_on,
        Timestamp::now(),
      ),
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
fn concurrent_gate(start: Start, tool_calls: bool) {
  let (folder, db, id, expected) = setup(start);
  let protected = gated(&expected, tool_calls);
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
    rusqlite::params![id.to_string(), serde_json::json!({"schema": Pipeline::SCHEMA, "body": &protected}).to_string(),
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
    .draft::<Pipeline>(id)
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
fn pipeline_draft_matching_refuses_concurrent_gates_on_unpublished_snapshot() {
  for tool_calls in [false, true] {
    concurrent_gate(Start::UnpublishedDraft, tool_calls);
  }
}
#[test]
fn pipeline_draft_matching_refuses_new_gated_draft_after_no_draft_snapshot() {
  for tool_calls in [false, true] {
    concurrent_gate(Start::PublishedOnly, tool_calls);
  }
}
#[test]
fn pipeline_draft_matching_refuses_concurrent_gates_on_existing_versioned_draft() {
  for tool_calls in [false, true] {
    concurrent_gate(Start::PublishedDraft, tool_calls);
  }
}
