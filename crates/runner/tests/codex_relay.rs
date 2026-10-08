//! The runner's end of the relay of Orchester's tools for Codex, end to end with `orchester-fake-codex`, whose model
//! chooses what it does to it (`fake_codex_relay`): only the two relays Codex starts are served, one for the thread
//! and one for the runner's `mcpServerStatus/list`, and a connection with the token beyond them is refused; a thread's
//! relay that comes after the inventory was answered is still taken, and one that never comes is said in the
//! activity; a connection that never shows the token holds no relay back, and connections with a wrong token keep
//! none out. Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test codex_relay
//! codex_relay`.

mod support;

use orchester_core::{
  connection::{mcp::McpServerProblem, stream::AgentEvent},
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  run::{Run, RunStatus},
  store::ActivityStore,
};
use orchester_store::ProjectDb;

use self::support::{Setup, project, reported, scheduler_with, wait_for};

/// A read-only Codex step on `model` that reads through Orchester's file tools, run to its end.
async fn finished(model: &'static str) -> (Run, ProjectDb) {
  let project = project(&Setup {
    model: Some(model),
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::AnyInWorktree,
      NetworkAccess::Off,
    )),
    ..Setup::new("Use the file tools.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id: RunId = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  (run, ProjectDb::open(root).expect("open"))
}

/// What the activity says of Orchester's server.
fn orchester_problems(db: &ProjectDb, run: &Run) -> Vec<McpServerProblem> {
  db.activity(run.id(), 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::McpServer {
        server: Some(server),
        problem,
      } if server == "Orchester" => Some(problem),
      _ => None,
    })
    .collect()
}

/// The thread's relay listed Orchester's tools and ran a call, and nothing reported Orchester's server
/// as not connected.
fn assert_tools_served(db: &ProjectDb, run: &Run) {
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let calls = reported(db, run.id(), "orchester-call: ");
  assert!(
    calls.first().is_some_and(|call| call.starts_with("read_file ok")),
    "{calls:#?}"
  );
  let problems = orchester_problems(db, run);
  assert!(problems.is_empty(), "{problems:#?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_relay_takes_only_the_two_relays_codex_starts() {
  let (run, db) = finished("fake-knock-model").await;
  assert_tools_served(&db, &run);
  assert_eq!(
    reported(&db, run.id(), "knock: "),
    ["closed"],
    "a third connection is refused, even with the token"
  );
  assert_eq!(reported(&db, run.id(), "late-knock: "), ["closed"]);
}

/// Codex may answer `mcpServerStatus/list` before the thread's relay came: the port still takes it.
#[tokio::test(flavor = "multi_thread")]
async fn codex_relay_takes_the_threads_relay_after_the_inventory_was_answered() {
  let (run, db) = finished("fake-late-relay-model").await;
  assert_tools_served(&db, &run);
}

/// A thread whose relay never came works without Orchester's tools, and the activity says so.
#[tokio::test(flavor = "multi_thread")]
async fn codex_relay_that_never_came_is_said_in_the_activity() {
  let (run, db) = finished("fake-no-thread-relay-model").await;
  assert_eq!(reported(&db, run.id(), "orchester-tools: "), ["no server"]);
  let detail =
    "Codex reached Orchester's tools 1 of the 2 times it starts them; the agent may have worked without them";
  assert_eq!(
    orchester_problems(&db, &run),
    [McpServerProblem::FailedToStart {
      detail: Some(detail.to_owned())
    }]
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_relay_is_not_held_back_by_a_silent_connection() {
  let (run, db) = finished("fake-silent-model").await;
  assert_tools_served(&db, &run);
}

/// Connections with a wrong token are refused one by one and never close the port before Codex's relays.
#[tokio::test(flavor = "multi_thread")]
async fn codex_relay_still_takes_its_relays_after_connections_with_a_wrong_token() {
  let (run, db) = finished("fake-bad-knocks-model").await;
  assert_eq!(reported(&db, run.id(), "bad-knock: "), ["closed"; 8]);
  assert_tools_served(&db, &run);
}
