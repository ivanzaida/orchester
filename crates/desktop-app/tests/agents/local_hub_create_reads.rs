//! What the create forms of a local project say when the choice of servers is refused, and when the project cannot be
//! told from a team's because its read failed (`Agents · v2` L3, ORC-843).
//! `cargo test -p orchester-app --test agents local_hub_create_reads`.

use orchester_app::{
  project::hold::ReadHold,
  routes::{AgentTab, HubFilter, ProjectPage, Route},
};
use orchester_core::connection::mcp::McpServerSelection;

use super::{
  harness::Agents,
  local_hub::server,
  local_hub_create::{form_route, show_form},
  local_hub_create_profile::run_on_codex,
  support::project_id,
};

/// What the servers row says under its label while the choice is refused.
const SERVERS_SAID: &str = "team-new-profile-mcp-sub";

/// A profile chooses at most `McpServerSelection::MAX` servers: the form says so under the row and makes nothing, and
/// the choice that fits is accepted again once one is taken off. (Choosing a server twice cannot be done in the form:
/// choosing it again takes it off, so `McpSelectionError::Duplicate` has no way in.)
#[test]
fn local_hub_create_a_profile_that_chooses_too_many_servers_says_so_under_the_row() {
  let mut agents = Agents::new(true);
  let ids: Vec<_> = (0..=McpServerSelection::MAX)
    .map(|number| server(&agents, &format!("docs-{number:02}")))
    .collect();
  // Tall enough that the servers' row under the role instructions is in the window, where a pointer presses it.
  agents.window.tree.resize(1440, 2400);
  show_form(&mut agents, HubFilter::Profiles);
  agents.set("team-new-profile-role-name-input", "Reviewer");
  run_on_codex(&mut agents, "test-model");

  for id in &ids[..McpServerSelection::MAX] {
    agents.window.press("local-hub-profile-add-mcp-button");
    let item = format!("local-hub-profile-mcp-{id}");
    agents.until("the menu", |s| s.exists(&item));
    agents.window.press(&item);
  }
  assert!(!agents.exists(SERVERS_SAID), "the most it takes is not refused");

  agents.window.press("local-hub-profile-add-mcp-button");
  let last = format!("local-hub-profile-mcp-{}", ids[McpServerSelection::MAX]);
  agents.until("the menu", |s| s.exists(&last));
  agents.window.press(&last);
  agents.until("the refusal", |s| {
    s.exists(SERVERS_SAID) && s.text(SERVERS_SAID) == "A profile chooses at most 20 MCP servers"
  });

  agents.click("local-hub-new-create");
  agents.until("a pass", |_| true);
  assert!(
    agents.listed().profiles.is_empty(),
    "a profile that chooses too many servers is not made"
  );

  agents.window.press("local-hub-profile-add-mcp-button");
  let first = format!("local-hub-profile-mcp-{}", ids[0]);
  agents.until("the menu", |s| s.exists(&first));
  agents.window.press(&first);
  agents.until("the refusal is gone", |s| !s.exists(SERVERS_SAID));
}

/// While the project's team read failed it is not known which hub the old New page stands for: the page says so, keeps
/// its route, and shows the form once the read goes through again.
#[test]
fn local_hub_create_the_old_new_page_says_so_when_the_team_read_failed() {
  let mut agents = Agents::new(true);
  let hold: ReadHold = agents.window.launch.services().expect("services").hold.clone();
  hold.set(true);
  let old = Route::Project(project_id(), ProjectPage::NewAgent(AgentTab::default()));
  agents.window.navigation().go(old.clone());
  agents.until("loading", |s| s.exists("team-hub-team-loading"));

  let db = rusqlite::Connection::open(agents.root.join(".orchester/orchester.db")).expect("fixture db");
  let version: u32 = db
    .query_row("PRAGMA user_version", [], |row| row.get(0))
    .expect("schema version");
  db.execute_batch("PRAGMA user_version = 99999").expect("future schema");
  hold.set(false);

  agents.until("the failure", |s| s.exists("team-hub-team-error"));
  assert!(agents.exists("team-hub-team-error-retry"));
  assert!(
    !agents.exists("local-hub-new-create"),
    "no form while the project is not known"
  );
  assert_eq!(agents.window.navigation().route_now(), old, "the route is not replaced");

  db.execute_batch(&format!("PRAGMA user_version = {version}"))
    .expect("restore fixture");
  agents.click("team-hub-team-error-retry");
  agents.until("the hub's form", |s| {
    s.window.navigation().route_now() == form_route(HubFilter::Profiles) && s.exists("local-hub-new-create")
  });
}
