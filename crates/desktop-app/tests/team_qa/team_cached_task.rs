//! A team task this computer holds only as the old sharing stream's cached copy and has no run of opens its page: the
//! page is the server's task, so it reads whether or not that copy is there, and keeps reading after a change that
//! leaves the copy in place (a team server smoke test found "The task couldn't be read" on such a task).
//!
//! `cargo test -p orchester-app --test team_qa team_qa_cached`

use std::num::NonZeroU32;

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  routes::{ProjectPage, Route, TaskView},
  test_support::{Data, Window, project_id},
};
use orchester_core::{
  id::TaskId,
  name::Name,
  project::TaskKey,
  sharing::{CachedTask, TaskRevision},
  store::{RunStore, TaskStore},
  task::{TaskLimits, TaskStatus, identity::TeamTaskRef, organization::TaskOrganization},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::team::{self, State};

/// The window on Kontur in `State::Member`, with KON-12 recorded as a team task and its old-stream copy cached, and no
/// run of it; on the task's page.
fn on_cached_kon_12() -> (Window, TeamServer) {
  let data = Data::new();
  let launch = data.launch();
  data.add_kontur(launch.workspace().expect("the session started"));
  let services = launch.services().expect("the session started");
  let server = team::prepare(State::Member, services, data.runtime.handle()).expect("the state is set up");
  let mut db = ProjectDb::open(&data.repo("repo-kontur")).expect("the project's database");
  let id: TaskId = server
    .authority_task_id(12)
    .expect("the stand-in serves KON-12")
    .parse()
    .expect("a task id");
  let link = db.sharing().expect("link").expect("linked");
  db.record_team_task(
    TeamTaskRef {
      server: link.server,
      project: link.project,
      task: id,
    },
    TaskRevision::try_from(1).expect("revision"),
  )
  .expect("identity");
  db.cache_shared_tasks(&[cached(id)], None).expect("cached");
  assert!(db.task(id).expect("read").is_some(), "the copy is there");
  assert!(db.runs_of_task(id).expect("runs").is_empty(), "and no run of it");
  drop(db);
  let window = Window::of(data, launch);
  let key: TaskKey = "KON-12".parse().expect("a task key");
  window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::task(key, TaskView::Summary, false),
  ));
  (window, server)
}

fn cached(id: TaskId) -> CachedTask {
  CachedTask {
    id,
    number: NonZeroU32::new(12).expect("number"),
    revision: TaskRevision::try_from(1).expect("revision"),
    title: Name::new("Cached title").expect("name"),
    brief: "Cached brief.".to_owned(),
    criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
    status: TaskStatus::Queued,
    organization: TaskOrganization::default(),
    module: None,
    former_key: None,
    created_by: "Alice Moreau".to_owned(),
    created_at: Timestamp::now(),
    updated_by: "Alice Moreau".to_owned(),
    updated_at: Timestamp::now(),
    claim: None,
    run: None,
  }
}

#[test]
fn team_qa_cached_a_team_task_with_a_cached_copy_and_no_run_opens_its_page() {
  let (mut window, _server) = on_cached_kon_12();
  window.until("the claim line or the failure", |window| {
    window.texts("task-team-claim-title").is_some() || window.texts("task-load-error").is_some()
  });

  assert!(window.texts("task-load-error").is_none(), "the page reads");
  // The server's task, not the copy that was cached.
  assert_eq!(window.text("task-title"), "Add retry to webhook sender");
  assert!(window.text("task-team-claim-title").starts_with("Claimed by you"));
}

#[test]
fn team_qa_cached_the_page_still_reads_after_a_change_the_cached_copy_survives() {
  let (mut window, _server) = on_cached_kon_12();
  window.until("the claim line", |window| window.texts("task-team-done").is_some());

  window.press("task-team-done");
  window.until("the task to be closed", |window| {
    window.texts("task-team-closed-title").is_some() || window.texts("task-load-error").is_some()
  });

  assert!(window.texts("task-load-error").is_none(), "the page reads");
  assert_eq!(window.text("task-team-closed-badge"), "Done");
}
