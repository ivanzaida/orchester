//! The runner's end of the relay of Orchester's tools for Codex, end to end with `orchester-fake-codex`, whose model
//! chooses what it does to it (`fake_codex_relay`): only the two relays Codex starts are served, one for the thread
//! and one for the runner's `mcpServerStatus/list`, and a connection with the token beyond them is refused; the port
//! closes once Codex's inventory was checked, even when fewer came; a connection that never shows the token holds
//! no relay back; and after the connections without the token the runner allows, the port refuses at once. Needs
//! `git`. Run with `cargo test -p orchester-runner --features test-agent --test codex_relay codex_relay`.

mod support;

use orchester_core::{
  connection::stream::AgentEvent,
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

/// The thread's relay listed Orchester's tools and ran a call, and nothing reported Orchester's server
/// as not connected.
fn assert_tools_served(db: &ProjectDb, run: &Run) {
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let calls = reported(db, run.id(), "orchester-call: ");
  assert!(
    calls.first().is_some_and(|call| call.starts_with("read_file ok")),
    "{calls:#?}"
  );
  let problems: Vec<AgentEvent> = db
    .activity(run.id(), 0)
    .expect("activity")
    .into_iter()
    .map(|entry| entry.event)
    .filter(|event| matches!(event, AgentEvent::McpServer { server: Some(server), .. } if server == "Orchester"))
    .collect();
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

#[tokio::test(flavor = "multi_thread")]
async fn codex_relay_closes_once_the_inventory_is_checked() {
  let (run, db) = finished("fake-one-relay-model").await;
  assert_tools_served(&db, &run);
  assert_eq!(
    reported(&db, run.id(), "late-knock: "),
    ["closed"],
    "only one relay came, but the turn's knock is refused"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_relay_is_not_held_back_by_a_silent_connection() {
  let (run, db) = finished("fake-silent-model").await;
  assert_tools_served(&db, &run);
}

/// After the connections without the token the runner allows, the port refuses a relay at once, which then
/// lists no tools, instead of keeping it waiting until Codex gives up.
#[tokio::test(flavor = "multi_thread")]
async fn codex_relay_refuses_at_once_after_too_many_connections_without_the_token() {
  let (run, db) = finished("fake-bad-knocks-model").await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(reported(&db, run.id(), "bad-knock: "), ["closed"; 8]);
  let problems: Vec<AgentEvent> = db
    .activity(run.id(), 0)
    .expect("activity")
    .into_iter()
    .map(|entry| entry.event)
    .filter(|event| matches!(event, AgentEvent::McpServer { .. }))
    .collect();
  assert!(problems.is_empty(), "no relay waited for an answer: {problems:#?}");
  assert_eq!(reported(&db, run.id(), "orchester-tools: "), [""]);
}
