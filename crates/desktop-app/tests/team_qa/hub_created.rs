//! A new MCP server made in the hub while the team server cannot be reached, and what becomes of its first version in
//! the outbox (`Agents · v2` A15 `dLqHC`, A16 `x3ywL`, ORC-835, ORC-836): it waits and can be cancelled from the panel;
//! a send that failed here after the server answered keeps it queued; one the server refused later, with the
//! person's work over it, stays listed, with the way out its waiting bar offers.
//!
//! `cargo test -p orchester-app --features test-support --test team_qa team_qa_hub_edit`

use std::time::{Duration, Instant};

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  routes::{HubFilter, HubView, ProjectPage, Route},
  test_support::Window,
};
use orchester_core::{id::ResourceId, resource::ResourceKind, store::ResourceStore};
use orchester_store::PublishState;
use orchester_workspace::{
  app_mcp::setup::{NewTeamMcpServer, Runs, create_team_mcp_server, team_mcp_names, team_mcp_server},
  sharing::NotCreated,
};

use super::{
  hub_edit::{db, fail_keeping_the_servers_number, on_pencil_until},
  team::State,
};

/// The panel's chip of the team's version, which every state of the panel has.
const TEAM_CHIP: &str = "team-hub-chip-team";

#[test]
fn team_qa_hub_edit_a_created_send_that_failed_here_after_the_server_may_have_answered_stays_queued() {
  let (window, server, _) = on_pencil_until(("team-edit", State::TeamEdit), TEAM_CHIP);
  let services = window.launch.services().expect("the session");
  let root = window.data.repo("repo-kontur");
  // The server is reached and numbers the version, then keeping its number fails on this computer: the send failed
  // here after the server answered.
  fail_keeping_the_servers_number(&window);
  let made = create_team_mcp_server(
    &root,
    &team_mcp_server(&new_server("fresh-four"), &team_mcp_names(&root).expect("names")).expect("a server"),
  )
  .expect("made");
  let created = window.data.runtime.block_on(services.servers.send_created(&root, made));
  assert!(matches!(created, Err(NotCreated::Failed(_))), "{created:?}");
  assert!(
    server
      .resources("kontur")
      .iter()
      .any(|(resource, _)| resource["id"] == made.to_string()),
    "the server holds the new resource"
  );
  assert_eq!(
    db(&window).waiting_publishes().expect("outbox").len(),
    1,
    "the version stays queued: a retry is idempotent, and the server holds it"
  );
  assert!(
    db(&window)
      .resources(ResourceKind::McpServer)
      .expect("resources")
      .iter()
      .any(|resource| resource.id() == made),
    "the resource was not taken back"
  );
}

#[test]
fn team_qa_hub_edit_a_created_resource_that_waits_is_cancelled_from_the_hub_panel() {
  let (mut window, _server, made) = waiting_created("fresh-one");
  assert!(window.texts("team-hub-edit").is_none());
  window.press("team-hub-edit-cancel");
  window.until("the new server to go", |window| {
    db(window)
      .resources(ResourceKind::McpServer)
      .expect("resources")
      .iter()
      .all(|resource| resource.id() != made)
  });
}

/// A new server `name` made here while the team server cannot be reached, so its first version waits in the outbox; the
/// window on the hub's panel of it.
fn waiting_created(name: &str) -> (Window, TeamServer, ResourceId) {
  let (mut window, server, (project, _)) = on_pencil_until(("team-edit", State::TeamEdit), TEAM_CHIP);
  server.set_down(true);
  let services = window.launch.services().expect("the session").clone();
  let root = window.data.repo("repo-kontur");
  let made = create_team_mcp_server(
    &root,
    &team_mcp_server(&new_server(name), &team_mcp_names(&root).expect("names")).expect("a server"),
  )
  .expect("made");
  let created = window
    .data
    .runtime
    .block_on(services.servers.send_created(&root, made))
    .expect("kept in the outbox");
  assert_eq!(created, orchester_workspace::sharing::Created::Waiting);
  window.until("the list to hold it", |window| {
    window.texts(&format!("team-hub-row-{made}-line")).is_some()
  });
  window.navigation().go(Route::Project(
    project,
    ProjectPage::hub(HubView::new(HubFilter::McpServers, Some(made))),
  ));
  window.until("the waiting panel of the new server", |window| {
    window.texts("team-hub-edit-waiting-title").is_some()
  });
  (window, server, made)
}

#[test]
fn team_qa_hub_edit_a_created_server_refused_later_with_work_over_it_stays_listed() {
  let (mut window, server, made) = waiting_created("fresh-two");
  let root = window.data.repo("repo-kontur");
  // A draft over the unsent first version: the person's work, which taking the resource back would lose.
  let mut fields = new_server("fresh-two");
  if let Runs::Program { arguments, .. } = &mut fields.runs {
    *arguments = vec!["--more".to_owned()];
  }
  orchester_workspace::agents::team_mcp_edit::save(
    &root,
    made,
    &fields,
    orchester_core::resource::VersionNumber::FIRST,
  )
  .expect("a draft over v1");
  server.reject_content("kontur", "body");
  server.set_down(false);
  let services = window.launch.services().expect("the session").clone();
  // The app's monitor may not have noticed the server is back: the pass that sends what waits is asked until it does.
  let deadline = Instant::now() + Duration::from_secs(20);
  while !matches!(
    db(&window).waiting_publishes().expect("outbox")[0].state,
    PublishState::Refused { .. }
  ) {
    window
      .data
      .runtime
      .block_on(services.servers.send_published(&root))
      .expect("sent");
    assert!(Instant::now() < deadline, "the server never refused the first version");
    std::thread::sleep(Duration::from_millis(100));
  }
  window.until("the row of the refused new server", |window| {
    window
      .texts(&format!("team-hub-row-{made}-line"))
      .is_some_and(|line| line.join(" ").starts_with("Not created · publish refused"))
  });
  window.until("its waiting bar", |window| {
    window.texts("team-hub-edit-refused-status").is_some()
  });
  // Discarding it would lose the draft: the press says why and nothing goes.
  window.press("team-hub-edit-discard-new");
  window.until("the reason", |window| {
    window.texts("team-hub-edit-send-error").is_some()
  });
  let said = window.text("team-hub-edit-send-error");
  assert!(said.contains("discard the draft first"), "{said}");
  assert!(
    db(&window)
      .resources(ResourceKind::McpServer)
      .expect("resources")
      .iter()
      .any(|resource| resource.id() == made),
    "the person's work is kept"
  );
}

#[test]
fn team_qa_hub_edit_a_refused_new_server_with_nothing_else_is_discarded_from_its_bar() {
  let (mut window, _server, made) = waiting_created("fresh-three");
  // A refusal the automatic take-back could not act on (a database error then): the version stays refused.
  let version = db(&window).waiting_publishes().expect("outbox")[0].version;
  db(&window)
    .record_publish_refused(version, "name_taken")
    .expect("recorded");
  let services = window.launch.services().expect("the session").clone();
  // The screens read the outbox again after a pass that sends what waits, which has nothing to send here.
  let root = window.data.repo("repo-kontur");
  window
    .data
    .runtime
    .block_on(services.servers.send_published(&root))
    .expect("a pass");
  window.until("its waiting bar", |window| {
    window.texts("team-hub-edit-refused-status").is_some()
  });
  window.press("team-hub-edit-discard-new");
  window.until("the new server to go", |window| {
    db(window)
      .resources(ResourceKind::McpServer)
      .expect("resources")
      .iter()
      .all(|resource| resource.id() != made)
  });
}

/// A new team MCP server of `name` that runs a program.
fn new_server(name: &str) -> NewTeamMcpServer {
  NewTeamMcpServer {
    name: name.to_owned(),
    runs: Runs::Program {
      program: "npx".to_owned(),
      arguments: vec!["fresh".to_owned()],
      folder: String::new(),
      variables: Vec::new(),
    },
    exempt_tools: Vec::new(),
  }
}
