//! The hub's create form against the stand-in team server (`Agents · New instruction` A10, ORC-835), on the QA app's
//! `team-admin` state: Create makes v1 and sends it at once, the form gives way to the new resource only once the
//! server has taken it, and a server that refuses it is said in place, with nothing made here.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_create`

use orchester_api::test_support::team_server::{Standing, TeamServer};
use orchester_app::{
  routes::{HubFilter, HubView, InstructionsTab, InstructionsView, ProjectPage, Route},
  test_support::{Window, project_id},
};
use orchester_core::{id::ResourceId, resource::ResourceKind, store::ResourceStore};
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
  let before = held(&window);
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
  assert_eq!(held(&window), before + 1);
  match window.navigation().route_now() {
    Route::Project(_, ProjectPage::AgentsHub(view)) => {
      assert_eq!(view.filter, HubFilter::Instructions);
      assert!(view.item.is_some() && !view.adding);
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
  assert!(
    window.text("team-new-refused").contains("not_admin"),
    "{}",
    window.text("team-new-refused")
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

/// A click on the element `id` itself, which the old editor's bar needs: it can lie below the window's fold.
fn click(window: &mut Window, id: &str) {
  window
    .tree
    .get_element_by_id_mut(id)
    .unwrap_or_else(|| panic!("{id} exists"))
    .click();
  window.pass();
}

/// The old Instructions page's new rule in the admin's window, filled: it says Create, not draft.
fn old_rule() -> (Window, TeamServer) {
  let (mut window, server) = prepared(State::Admin);
  window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Instructions(InstructionsView::new_rule(InstructionsTab::Project)),
  ));
  window.until("the editor", |window| window.texts("instruction-create").is_some());
  assert_eq!(window.text("instruction-create"), "Create");
  window.set("instruction-name", "Working agreement");
  window.set("instruction-prompt", "Follow the approved plan.");
  (window, server)
}

#[test]
fn team_qa_create_the_old_instruction_window_sends_v1_at_once_and_says_create() {
  let (mut window, server) = old_rule();
  click(&mut window, "instruction-create");
  window.until("the server to take it", |_| {
    server
      .resources("kontur")
      .iter()
      .any(|(resource, versions)| resource["name"] == "Working agreement" && versions.len() == 1)
  });
}

#[test]
fn team_qa_create_the_old_instruction_window_makes_nothing_the_server_refused() {
  let (mut window, server) = old_rule();
  let before = held(&window);
  server.stand_as(Standing::Member);
  click(&mut window, "instruction-create");
  // Said in the editor, and the refused rule is taken back whole: it is not the Instructions page's.
  window.until("the refusal", |window| {
    window.texts("instruction-write-error").is_some()
  });
  assert!(window.text("instruction-write-error").contains("not_admin"));
  assert_eq!(held(&window), before);
  assert!(
    !server
      .resources("kontur")
      .iter()
      .any(|(resource, _)| resource["name"] == "Working agreement")
  );
}

const WAITING_LINE: &str = "Waiting to send to the team server";

/// The id of the instruction `name` this computer holds.
fn instruction(window: &Window, name: &str) -> ResourceId {
  ProjectDb::open(&window.data.repo("repo-kontur"))
    .expect("database")
    .resources(ResourceKind::Instruction)
    .expect("resources")
    .iter()
    .find(|resource| resource.name().as_str() == name)
    .unwrap_or_else(|| panic!("{name} is held"))
    .id()
}

fn instructions_list() -> Route {
  Route::Project(
    project_id(),
    ProjectPage::hub(HubView::new(HubFilter::Instructions, None)),
  )
}

/// The waiting instruction `name` listed in the hub, its row's line read.
fn waiting_row(window: &mut Window, name: &str) -> String {
  let row = format!("team-hub-row-{}-line", instruction(window, name));
  window.navigation().go(instructions_list());
  window.until("its row", |window| {
    window.texts(&row).is_some_and(|text| text.join(" ") == WAITING_LINE)
  });
  window.text(&row)
}

/// Sends what waits to the team server, as it does once the server answers.
fn send_waiting(window: &Window) {
  let services = window.launch.services().expect("the session started");
  let root = window.data.repo("repo-kontur");
  // The refusal of a version, if any, is what the hub tells; this is only the send.
  let _ = window.data.runtime.block_on(services.servers.send_published(&root));
}

#[test]
fn team_qa_create_a_server_that_cannot_be_reached_keeps_it_waiting_to_send_and_sends_it_when_back() {
  let (mut window, server) = filled(State::Admin);
  let before = held(&window);
  server.set_down(true);
  window.press("team-hub-new-create");
  window.until("the waiting note", |window| window.texts("team-new-waiting").is_some());
  assert!(window.text("team-new-waiting").contains("waiting to send"));
  assert_eq!(held(&window), before + 1, "this computer keeps it");
  assert_eq!(
    waiting_row(&mut window, "Release notes"),
    WAITING_LINE,
    "the list shows it as waiting, not as published"
  );
  server.set_down(false);
  // The session sends it on reconnecting; here it is sent until the session has seen the server answer.
  window.until("the server to take it", |window| {
    send_waiting(window);
    server
      .resources("kontur")
      .iter()
      .any(|(resource, versions)| resource["name"] == "Release notes" && versions.len() == 1)
  });
  let row = format!("team-hub-row-{}-line", instruction(&window, "Release notes"));
  window.until("it is no longer waiting", |window| {
    window.texts(&row).is_some_and(|text| text[0] != WAITING_LINE)
  });
}

#[test]
fn team_qa_create_a_waiting_one_the_server_refuses_later_is_taken_back_and_the_hub_says_why() {
  let (mut window, server) = filled(State::Admin);
  let before = held(&window);
  server.set_down(true);
  window.press("team-hub-new-create");
  window.until("the waiting note", |window| window.texts("team-new-waiting").is_some());
  assert_eq!(waiting_row(&mut window, "Release notes"), WAITING_LINE);
  // The server is back and no longer takes this person's writes.
  server.stand_as(Standing::Member);
  server.set_down(false);
  window.until("the notice", |window| {
    send_waiting(window);
    window.texts("team-hub-taken-back-0").is_some()
  });
  assert_eq!(held(&window), before, "a refused new resource is taken back whole");
  assert!(window.text("team-hub-taken-back-0").contains("Release notes"));
  window.press("team-hub-taken-back-dismiss-0");
  window.until("the notice gone", |window| {
    window.texts("team-hub-taken-back-0").is_none()
  });
}

#[test]
fn team_qa_create_the_old_instruction_window_keeps_it_waiting_and_it_is_sent_when_the_server_is_back() {
  let (mut window, server) = old_rule();
  let before = held(&window);
  server.set_down(true);
  click(&mut window, "instruction-create");
  window.until("it is kept", |window| held(window) == before + 1);
  let outbox = ProjectDb::open(&window.data.repo("repo-kontur"))
    .expect("database")
    .waiting_publishes()
    .expect("the outbox");
  assert_eq!(outbox.len(), 1, "its v1 waits in the outbox");
  assert!(
    server
      .resources("kontur")
      .iter()
      .all(|(resource, _)| resource["name"] != "Working agreement")
  );
  server.set_down(false);
  window.until("the server to take it", |window| {
    send_waiting(window);
    server
      .resources("kontur")
      .iter()
      .any(|(resource, versions)| resource["name"] == "Working agreement" && versions.len() == 1)
  });
}
