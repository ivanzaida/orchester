//! The hub's create form against the stand-in team server (`Agents · New instruction` A10, ORC-835), on the QA app's
//! `team-admin` state: Create makes v1 and sends it at once, the form gives way to the new resource only once the
//! server has taken it, and a server that refuses it is said in place, with nothing made here.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_create`

use orchester_api::test_support::team_server::{Standing, TeamServer};
use orchester_app::{
  routes::{HubFilter, HubView, ProjectPage, Route},
  test_support::{Window, project_id},
};
use orchester_core::{resource::ResourceKind, store::ResourceStore};
use orchester_store::ProjectDb;

use super::{prepared, team::State};

fn form() -> Route {
  Route::Project(project_id(), ProjectPage::hub(HubView::adding(HubFilter::Instructions)))
}

/// The instruction's form in `state`, filled with a name and a text.
fn filled(state: State) -> (Window, TeamServer) {
  let (mut window, server) = prepared(state);
  window.navigation().go(form());
  window.until("the form", |window| window.texts("team-hub-new-create").is_some());
  window.set("team-new-instruction-name", "Release notes");
  window.set("team-new-instruction-text", "Write them for customers.");
  (window, server)
}

/// How many instructions this computer holds.
fn held(window: &Window) -> usize {
  ProjectDb::open(&window.data.repo("repo-kontur"))
    .expect("database")
    .resources(ResourceKind::Instruction)
    .expect("resources")
    .len()
}

#[test]
fn team_qa_create_sends_v1_at_once_and_shows_the_resource_once_the_server_has_it() {
  let (mut window, server) = filled(State::Admin);
  window.press("team-hub-new-create");
  window.until("the server to take it", |_| {
    server
      .resources("kontur")
      .iter()
      .any(|(resource, versions)| resource["name"] == "Release notes" && versions.len() == 1)
  });
  // The form gives way to the new resource's panel, which names it and its first version.
  window.until("the new resource", |window| {
    window.texts("team-hub-new-create").is_none() && window.texts("team-hub-panel-title").is_some()
  });
  assert_eq!(window.text("team-hub-panel-title"), "Release notes");
  match window.navigation().route_now() {
    Route::Project(_, ProjectPage::AgentsHub(view)) => {
      assert_eq!(view.filter, HubFilter::Instructions);
      assert!(view.item.is_some() && !view.adding, "{view:?}");
    }
    other => panic!("the hub on the new instruction, not {other:?}"),
  }
}

#[test]
fn team_qa_create_a_server_that_refuses_it_is_said_in_place_and_nothing_is_made() {
  let (mut window, server) = filled(State::Admin);
  let before = held(&window);
  // The server no longer takes this person's writes; this computer has not read that yet.
  server.stand_as(Standing::Member);
  window.press("team-hub-new-create");
  window.until("the refusal", |window| window.texts("team-new-refused").is_some());
  assert_eq!(
    window.text("team-new-refused-title"),
    "The team's server refused: you are no longer an admin"
  );
  assert_eq!(window.text("team-hub-panel-title"), "New instruction");
  assert!(
    window.texts("team-hub-new-create").is_some(),
    "the form stays, to try again or cancel"
  );
  assert_eq!(
    held(&window),
    before,
    "a resource the server refused is taken back whole"
  );
  assert!(
    !server
      .resources("kontur")
      .iter()
      .any(|(resource, _)| resource["name"] == "Release notes")
  );
}

/// The form of `filter` in the admin's window, its fields set, and Create pressed: the name the server then holds.
fn made(filter: HubFilter, fields: &[(&str, &str)], name: &str) -> (Window, TeamServer) {
  let (mut window, server) = prepared(State::Admin);
  window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::hub(HubView::adding(filter))));
  window.until("the form", |window| window.texts("team-hub-new-create").is_some());
  for (id, value) in fields {
    window.set(id, value);
  }
  window.press("team-hub-new-create");
  window.until("the server to take it", |_| {
    server
      .resources("kontur")
      .iter()
      .any(|(resource, versions)| resource["name"] == name && versions.len() == 1)
  });
  (window, server)
}

#[test]
fn team_qa_create_a_skill_a_tool_and_an_mcp_server_are_the_servers_at_v1() {
  let (_, server) = made(
    HubFilter::Skills,
    &[
      ("team-new-skill-name", "review-diff"),
      ("team-new-skill-description", "Review a diff before it merges."),
    ],
    "review-diff",
  );
  let held = server.resources("kontur");
  let skill = held
    .iter()
    .find(|(resource, _)| resource["name"] == "review-diff")
    .expect("the skill");
  assert_eq!(skill.0["kind"], "skill");

  let program = std::env::current_exe()
    .expect("a program")
    .to_string_lossy()
    .into_owned();
  let (_, server) = made(
    HubFilter::Tools,
    &[
      ("team-new-tool-name", "cargo-check"),
      ("team-new-tool-program", &program),
    ],
    "cargo-check",
  );
  assert!(
    server
      .resources("kontur")
      .iter()
      .any(|(resource, _)| resource["kind"] == "tool")
  );

  let (_, server) = made(
    HubFilter::McpServers,
    &[("team-new-mcp-name", "figma"), ("team-new-mcp-program", "npx")],
    "figma",
  );
  let held = server.resources("kontur");
  let figma = held
    .iter()
    .find(|(resource, _)| resource["name"] == "figma")
    .expect("the server");
  assert_eq!(figma.0["kind"], "mcp_server");
  // The team holds the program's name and nothing of this computer.
  assert_eq!(figma.1[0]["body"]["launch"]["program"], "npx");
}
