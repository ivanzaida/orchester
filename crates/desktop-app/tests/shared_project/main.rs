//! Project settings › Sharing once linked (N2) in the window, against a real team server on loopback behind a
//! recording port: the first checkout linked to an empty project uploads its tasks with their numbers on the `KSnOW`
//! page, a later checkout gets new numbers, members are read-only with *Manage members and invites* opening the
//! server admin (links recorded, no browser), *Make local* keeps every task, `orchester.toml` is contacted only after
//! the person confirms it, and a conflicting edit of a shared task shows the field-by-field view with the server's
//! names. No test calls an endpoint of the server admin.
//!
//! `cargo test -p orchester-app --test shared_project shared_project_`

mod conflict;
mod discovery;
#[allow(dead_code, reason = "the N1 harness, of which these tests use a part")]
#[path = "../orchester/harness.rs"]
mod harness;
mod linking;
mod make_local;
mod members;
#[path = "../../../workspace/tests/server_support/mod.rs"]
mod server_support;
#[path = "../support/mod.rs"]
mod support;
mod task_list;

use std::path::PathBuf;

use harness::{Recorder, administrative, block_on, press, shows};
use orchester_api::{
  SharedProject,
  client::{PlainHttp, ServerAddress},
};
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  store::TaskStore,
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::sharing::{CodeRequest, ServerEntry, Trust, project::LinkFirst};
use server_support::TestServer;
use support::Window;

const KONTUR_REMOTE: &str = "https://github.com/acme/kontur.git";

/// A plain team server with Kontur (KON, empty, set up for [`KONTUR_REMOTE`]) that Alice Moreau, its first admin,
/// made; Alice owns it.
fn server() -> (TestServer, SharedProject) {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let kontur = server.create_named_project(("Kontur", "KON"), "kontur", Some(KONTUR_REMOTE));
  (server, kontur)
}

/// `username` joins `kontur` from the window's computer with a project code, through a recording port in front of
/// `server`; the port.
fn join(window: &Window, server: &TestServer, kontur: &SharedProject, (username, name): (&str, &str)) -> Recorder {
  let code = server.project_code(kontur);
  let recorder = Recorder::start(server.address);
  let services = window.launch.services().expect("services").clone();
  let address = ServerAddress::parse(&recorder.url()).expect("address");
  let request = CodeRequest {
    code,
    username: Some(username.into()),
    display_name: Some(name.into()),
    proof: Some(server.code_proof()),
  };
  block_on(
    window,
    services
      .servers
      .join_with_code(&address, Trust::System(PlainHttp::NotAllowed), request),
  )
  .expect("join with a code");
  recorder
}

/// The window's Kontur checkout.
fn kontur_root(window: &Window) -> PathBuf {
  window.data.repo("repo-kontur")
}

/// Local draft tasks titled `titles` in the window's Kontur.
fn local_tasks(window: &Window, titles: &[&str]) {
  let mut db = ProjectDb::open(&kontur_root(window)).expect("Kontur's database");
  let details = titles
    .iter()
    .map(|title| TaskDetails {
      title: Name::new(title).expect("a title"),
      brief: String::new(),
      acceptance_criteria: Vec::new(),
      pipeline: None,
      limits: TaskLimits::new(None, None).expect("no limits"),
    })
    .collect();
  db.create_tasks(IdempotencyKey::generate(), details, Timestamp::now())
    .expect("local tasks");
}

/// The keys of the window's Kontur's tasks, by number.
fn local_keys(window: &Window) -> Vec<String> {
  let db = ProjectDb::open(&kontur_root(window)).expect("Kontur's database");
  let mut tasks = db.tasks().expect("tasks");
  tasks.sort_by_key(|task| task.key().number());
  tasks.iter().map(|task| task.key().to_string()).collect()
}

/// The one server the window joined.
fn entry(window: &Window) -> ServerEntry {
  let services = window.launch.services().expect("services");
  services.servers.entries().expect("entries").remove(0)
}

/// Links the window's Kontur to Kontur on its server through the workspace, as the first checkout.
fn link_kontur(window: &Window) {
  let services = window.launch.services().expect("services").clone();
  let request = LinkFirst {
    slug: "kontur".into(),
    remote: None,
    key: IdempotencyKey::generate(),
    write_discovery: false,
  };
  block_on(
    window,
    services
      .servers
      .link_first(&entry(window), &kontur_root(window), request),
  )
  .expect("linked");
}

/// Project settings › Sharing of the window's Kontur, once `id` shows.
fn open_sharing(window: &mut Window, id: &str) {
  harness::go(
    window,
    Route::Project(support::project_id(), ProjectPage::Settings),
    "settings-tab-sharing",
  );
  press(window, "settings-tab-sharing");
  shows(window, id);
}

/// No request through `recorder` named an endpoint only the server admin calls.
fn no_admin_endpoint(recorder: &Recorder) {
  let lines = recorder.lines();
  assert!(
    !lines.iter().any(|line| administrative(line)),
    "an admin endpoint was called: {lines:?}"
  );
}
