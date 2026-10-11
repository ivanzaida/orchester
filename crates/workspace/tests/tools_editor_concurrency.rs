//! A writer commits only after SQLite confirms that the editor is awaiting its write lock.
mod support;

use std::{cell::RefCell, sync::mpsc, thread, time::Duration};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Draft, Tool, ToolAction, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  connections::mcp::Allowed,
  tools::{
    editor::{self, Edit, EditorData},
    form::CommandText,
  },
};
use support::Fixture;

struct WriteWait {
  entered: mpsc::Sender<()>,
  committed: mpsc::Receiver<()>,
}
thread_local! {
  static WRITE_WAIT: RefCell<Option<WriteWait>> = const { RefCell::new(None) };
}
fn await_writer(_attempt: i32) -> bool {
  WRITE_WAIT.with_borrow_mut(|wait| {
    let wait = wait.take().expect("this editor awaits the writer exactly once");
    wait.entered.send(()).expect("observe actual write contention");
    wait
      .committed
      .recv_timeout(Duration::from_secs(10))
      .expect("writer committed");
    true
  })
}

#[derive(Clone, Copy)]
enum Action {
  Save,
  KeepDraft,
  StartLatest,
  Publish,
}
#[derive(Clone, Copy)]
enum OutsideChange {
  DraftBody,
  LatestVersion,
}

fn initial_tool() -> Tool {
  CommandText {
    program: std::env::current_exe()
      .expect("test binary")
      .to_string_lossy()
      .into_owned(),
    description: "original".into(),
    arguments: "before".into(),
    ..CommandText::new_tool()
  }
  .tool(None)
  .expect("command")
}
fn setup(action: Action) -> (Fixture, ResourceId, Edit) {
  let mut f = Fixture::new();
  let name = Name::new("Checks").expect("name");
  let tool = initial_tool();
  let id = orchester_workspace::tools::data::create_tool(&mut f.db, &name, &tool, Timestamp::now()).expect("tool");
  f.db.publish_draft::<Tool>(id, Timestamp::now()).expect("published");
  if !matches!(action, Action::Save) {
    let draft = Tool::new("editor draft", tool.action().clone(), None).expect("command");
    f.db
      .save_draft(id, &Draft::new(draft, Some(VersionNumber::FIRST), Timestamp::now()))
      .expect("draft");
  }
  let entry = EditorData::load_in(f.repo.path(), Some(id))
    .expect("read")
    .listed
    .tool(id)
    .expect("entry")
    .clone();
  let edit = match action {
    Action::Save => Edit::Save {
      name,
      tool: Box::new(Tool::new("local harmless edit", tool.action().clone(), None).expect("command")),
      before: Some(entry),
    },
    Action::KeepDraft => Edit::KeepDraft(entry),
    Action::StartLatest => Edit::StartLatest(entry),
    Action::Publish => Edit::Publish(entry),
  };
  (f, id, edit)
}

fn concurrent_change(action: Action, change: OutsideChange) {
  let (f, id, edit) = setup(action);
  let target = f.repo.path().to_path_buf();
  let (ready_tx, ready_rx) = mpsc::channel();
  let (start_tx, start_rx) = mpsc::channel();
  let (wait_tx, wait_rx) = mpsc::channel();
  let (commit_tx, commit_rx) = mpsc::channel();
  let worker = thread::spawn(move || {
    let mut db = ProjectDb::open(&target).expect("editor connection");
    WRITE_WAIT.set(Some(WriteWait {
      entered: wait_tx,
      committed: commit_rx,
    }));
    db.set_busy_handler_for_test(await_writer).expect("observe write wait");
    ready_tx.send(()).expect("ready");
    start_rx.recv_timeout(Duration::from_secs(10)).expect("writer locked");
    editor::apply_with_db_for_test(&mut db, &target, edit, &Allowed::These(vec![]))
  });
  ready_rx
    .recv_timeout(Duration::from_secs(10))
    .expect("connection ready");
  let writer = rusqlite::Connection::open(f.repo.path().join(".orchester/orchester.db")).expect("writer");
  writer.execute_batch("BEGIN IMMEDIATE").expect("writer lock");
  start_tx.send(()).expect("start editor");
  wait_rx
    .recv_timeout(Duration::from_secs(10))
    .expect("editor reached write lock");
  overwrite(&writer, id, change);
  writer.execute_batch("COMMIT").expect("outside writer commit");
  commit_tx.send(()).expect("allow editor lock acquisition");
  let result = worker.join().expect("editor thread");
  assert_eq!(
    result.expect_err("must preserve concurrent state").key,
    "errors.draftChanged"
  );
  match change {
    OutsideChange::DraftBody => {
      let draft = f.db.draft::<Tool>(id).expect("read").expect("outside draft");
      assert_eq!(draft.body().description(), "outside writer");
      let ToolAction::Command(command) = draft.body().action() else {
        panic!("command");
      };
      assert_eq!(command.args()[0].as_str(), "after");
      assert_eq!(f.db.versions::<Tool>(id).expect("versions").len(), 1);
    }
    OutsideChange::LatestVersion => {
      assert_eq!(
        f.db
          .latest_version::<Tool>(id)
          .expect("latest")
          .expect("v2")
          .number()
          .get(),
        2
      );
      let draft = f.db.draft::<Tool>(id).expect("draft");
      if matches!(action, Action::Save) {
        assert!(draft.is_none(), "Save did not recreate the old version as a draft");
      } else {
        let draft = draft.expect("local draft unchanged");
        assert_eq!(draft.body().description(), "editor draft");
        assert_eq!(draft.based_on(), Some(VersionNumber::FIRST));
      }
    }
  }
}

fn overwrite(writer: &rusqlite::Connection, id: ResourceId, change: OutsideChange) {
  let body: String = writer
    .query_row(
      "SELECT body FROM resource_versions WHERE resource_id=?1 AND number=1",
      [id.to_string()],
      |row| row.get(0),
    )
    .expect("stored body");
  match change {
    OutsideChange::DraftBody => {
      let body = body
        .replace("\"original\"", "\"outside writer\"")
        .replace("\"before\"", "\"after\"");
      writer
        .execute(
          "INSERT INTO resource_drafts(resource_id,body,based_on,updated_at) VALUES(?1,?2,1,?3) \
         ON CONFLICT(resource_id) DO UPDATE SET body=excluded.body,updated_at=excluded.updated_at",
          rusqlite::params![id.to_string(), body, Timestamp::now().unix_millis()],
        )
        .expect("external draft");
    }
    OutsideChange::LatestVersion => {
      writer
        .execute(
          "INSERT INTO resource_versions(id,resource_id,number,body,published_at) VALUES(?1,?2,2,?3,?4)",
          rusqlite::params![
            orchester_core::id::VersionId::generate().to_string(),
            id.to_string(),
            body,
            Timestamp::now().unix_millis()
          ],
        )
        .expect("external publication");
    }
  }
}

#[test]
fn tools_editor_save_refuses_writer_committing_while_waiting_for_write_lock() {
  concurrent_change(Action::Save, OutsideChange::DraftBody);
  concurrent_change(Action::Save, OutsideChange::LatestVersion);
}
#[test]
fn tools_editor_keep_draft_refuses_writer_committing_while_waiting_for_write_lock() {
  concurrent_change(Action::KeepDraft, OutsideChange::DraftBody);
  concurrent_change(Action::KeepDraft, OutsideChange::LatestVersion);
}
#[test]
fn tools_editor_start_latest_refuses_writer_committing_while_waiting_for_write_lock() {
  concurrent_change(Action::StartLatest, OutsideChange::DraftBody);
  concurrent_change(Action::StartLatest, OutsideChange::LatestVersion);
}
#[test]
fn tools_editor_publish_refuses_writer_committing_while_waiting_for_write_lock() {
  concurrent_change(Action::Publish, OutsideChange::DraftBody);
  concurrent_change(Action::Publish, OutsideChange::LatestVersion);
}
#[test]
fn tools_editor_stale_resolution_keeps_draft_or_starts_latest() {
  for keep in [true, false] {
    let (f, id, _) = setup(Action::KeepDraft);
    let writer = rusqlite::Connection::open(f.repo.path().join(".orchester/orchester.db")).expect("writer");
    overwrite(&writer, id, OutsideChange::LatestVersion);
    let entry = EditorData::load_in(f.repo.path(), Some(id))
      .expect("read")
      .listed
      .tool(id)
      .expect("entry")
      .clone();
    let edit = if keep {
      Edit::KeepDraft(entry)
    } else {
      Edit::StartLatest(entry)
    };
    editor::apply_in(f.repo.path(), edit, &Allowed::These(vec![])).expect("resolve stale draft");
    let draft = f.db.draft::<Tool>(id).expect("draft");
    if keep {
      let draft = draft.expect("kept");
      assert_eq!(draft.body().description(), "editor draft");
      assert_eq!(draft.based_on().expect("v2").get(), 2);
    } else {
      assert!(draft.is_none(), "starting from latest discards the redundant draft");
    }
  }
}
