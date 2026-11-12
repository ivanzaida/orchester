//! An admin's edit of a team MCP server in the Agents hub, and what the team server answers, against the stand-in team
//! server (`Agents · v2` A13 `WMtxm`, `docs/design/team-configuration-live.md` rule 6): *Edit* makes the team's part a
//! form under the fixed Save/Cancel bar, *Save* publishes the next version on the team server at once and the panel
//! says the answer in place, a refusal keeps what was typed, and nothing is kept on this computer; while the team
//! server cannot be reached, Save saves nothing and an open form keeps what was typed. On the QA states
//! `team-edit`, `team-edit-demoted`, `team-edit-offline` and `team-edit-rejects`.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_hub_edit`

use lurq::app::events::with_synthetic_input;
use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  routes::{HubFilter, HubView, ProjectPage, Route},
  test_support::{Data, Window},
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::ResourceKind,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use tempfile::TempDir;
use tokio::runtime::Runtime;

use super::team::{self, State};

const ARGUMENTS: &str = "team-new-mcp-arguments";
const SAVE: &str = "team-hub-edit-save";
const CANCEL: &str = "team-hub-edit-cancel";

/// The window on the seed's `scenario` joined to the stand-in team server in `state`, on the hub's MCP servers with
/// `pencil-design` open, once its form shows: an admin edits it in place.
fn on_pencil(of: (&str, State)) -> (Window, TeamServer, (ProjectId, ResourceId)) {
  on_pencil_until(of, ARGUMENTS)
}

/// [`on_pencil`], once the panel's element `shown` has its text.
pub(super) fn on_pencil_until(of: (&str, State), shown: &str) -> (Window, TeamServer, (ProjectId, ResourceId)) {
  let (window, server, (project, pencil)) = opened(of, shown, |window| {
    let pencil = db(window)
      .resources(ResourceKind::McpServer)
      .expect("resources")
      .into_iter()
      .find(|resource| resource.name().as_str() == "pencil-design")
      .expect("the seed's pencil-design")
      .id();
    (HubView::new(HubFilter::McpServers, Some(pencil)), Some(pencil))
  });
  (window, server, (project, pencil.expect("pencil-design")))
}

/// The window on the seed's `scenario` joined to the stand-in team server in `state`, on the hub's view `view`
/// picks (with the resource it names), once the panel's element `shown` has its text.
pub(super) fn opened(
  (scenario, state): (&str, State),
  shown: &str,
  view: impl FnOnce(&Window) -> (HubView, Option<ResourceId>),
) -> (Window, TeamServer, (ProjectId, Option<ResourceId>)) {
  let dir = TempDir::new().expect("a temporary folder");
  let request = crate::kontur::SeedRequest {
    data_dir: dir.path().to_path_buf(),
    repo: dir.path().join("repo-kontur"),
    scenario: crate::kontur::scenarios::named(scenario).expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = crate::kontur::seed(&request).expect("the scenario seeds");
  let data = Data::at(dir, Runtime::new().expect("tokio runtime"));
  let launch = data.launch();
  let services = launch.services().expect("the session started");
  let server = team::prepare(state, services, data.runtime.handle()).expect("the state is set up");
  let mut window = Window::of(data, launch);
  let (view, picked) = view(&window);
  window
    .navigation()
    .go(Route::Project(seeded.project, ProjectPage::hub(view)));
  window.until("the panel", |window| window.texts(shown).is_some());
  (window, server, (seeded.project, picked))
}

pub(super) fn db(window: &Window) -> ProjectDb {
  ProjectDb::open(&window.data.repo("repo-kontur")).expect("database")
}

/// The arguments typed in the form.
fn typed(window: &mut Window) {
  window.set(ARGUMENTS, "--stdio\n--app\ndesktop\n--no-telemetry");
  window.until("Save to say what it makes", |window| {
    window
      .texts("team-hub-actions-hint")
      .is_some_and(|said| said.join(" ").contains("v2"))
  });
}

/// Whether the form holds nothing to save: the bar is the fix of what this computer lacks, or says nothing changed.
fn unchanged(window: &mut Window) -> bool {
  window.texts(SAVE).is_none()
    || window
      .texts("team-hub-actions-hint")
      .is_some_and(|said| said.join(" ").starts_with("No changes"))
}

/// Whether the stand-in server holds one version of every MCP server: nothing was published.
fn nothing_published(server: &TeamServer) -> bool {
  server
    .resources("kontur")
    .iter()
    .all(|(_, versions)| versions.len() == 1)
}

#[test]
fn team_qa_hub_edit_save_publishes_the_next_version_on_the_team_server_at_once() {
  let (mut window, server, _) = on_pencil(("team-edit", State::TeamEdit));
  typed(&mut window);
  assert!(window.texts(CANCEL).is_some(), "Cancel stands beside Save");

  window.press(SAVE);
  window.until("the server to number it", |_| {
    server
      .resources("kontur")
      .iter()
      .any(|(resource, versions)| resource["name"] == "pencil-design" && versions.len() == 2)
  });
  window.until("the panel to read as published", unchanged);
  window.until("the team's version", |window| {
    window.text("team-hub-chip-team") == "Team · v2"
  });
}

#[test]
fn team_qa_hub_edit_cancel_puts_back_the_version() {
  let (mut window, server, _) = on_pencil(("team-edit", State::TeamEdit));
  typed(&mut window);
  window.press(CANCEL);
  window.until("the version back in the form", unchanged);
  let arguments = window.texts(ARGUMENTS).expect("the arguments").join(" ");
  assert!(!arguments.contains("--no-telemetry"), "{arguments}");
  assert!(nothing_published(&server));
}

#[test]
fn team_qa_hub_edit_no_longer_an_admin_is_said_in_place_and_keeps_what_was_typed() {
  let (mut window, server, _) = on_pencil(("team-edit-demoted", State::TeamEditDemoted));
  typed(&mut window);
  window.press(SAVE);
  window.until("the refusal", |window| window.texts("team-hub-edit-refused").is_some());
  let body = window.text("team-hub-edit-refused-body");
  assert!(
    body.contains("Only the project's owner or the server's admin"),
    "{body}"
  );
  assert!(window.texts(SAVE).is_some(), "the form stays with what was typed");
  assert!(nothing_published(&server));
}

/// Whether the hub's head says the team server cannot be reached.
pub(super) fn says_unreachable(window: &mut Window) -> bool {
  window
    .texts("team-hub-team")
    .is_some_and(|said| said.join(" ").contains("Can't reach the team"))
}

#[test]
fn team_qa_hub_edit_offline_saves_nothing_and_keeps_what_was_typed() {
  let (mut window, server, _) = on_pencil(("team-edit", State::TeamEdit));
  typed(&mut window);
  server.set_down(true);
  window.until("the hub to say it cannot reach the team", says_unreachable);
  window.press(SAVE);
  assert_eq!(
    window.text("team-hub-chip-team"),
    "Team · v1",
    "the team's version is still v1"
  );
  assert!(window.texts(SAVE).is_some(), "the form stays with what was typed");
  let arguments = window.texts(ARGUMENTS).expect("the arguments").join(" ");
  assert!(arguments.contains("--no-telemetry"), "{arguments}");
  assert!(nothing_published(&server));
}

#[test]
fn team_qa_hub_edit_offline_state_shows_what_was_read_and_saves_nothing() {
  let (mut window, server, _) = on_pencil_until(("team-edit-offline", State::TeamEditOffline), "team-hub-panel-title");
  window.until("the hub to say it cannot reach the team", says_unreachable);
  assert_eq!(window.text("team-hub-panel-title"), "pencil-design");
  assert!(
    unchanged(&mut window),
    "nothing is changed while the team cannot be reached"
  );
  assert!(nothing_published(&server));
}

#[test]
fn team_qa_hub_edit_a_rejected_content_says_why_in_place_and_keeps_what_was_typed() {
  let (mut window, _server, _) = on_pencil(("team-edit-rejects", State::TeamEditRejects));
  typed(&mut window);
  window.press(SAVE);
  window.until("the refusal", |window| window.texts("team-hub-edit-refused").is_some());
  assert_eq!(
    window.text("team-hub-edit-refused-title"),
    "The team's server refused v2"
  );
  let body = window.text("team-hub-edit-refused-body");
  assert!(body.contains("does not accept its content"), "{body}");
  assert!(body.contains("What you typed is kept"), "{body}");
  assert!(window.texts(SAVE).is_some(), "the form stays, to fix it and save again");
}

#[test]
fn team_qa_hub_edit_create_a_name_the_team_server_has_is_a_field_error_and_nothing_is_made() {
  let (mut window, _server, (project, _)) = on_pencil(("team-edit", State::TeamEdit));
  let before = db(&window).resources(ResourceKind::McpServer).expect("resources").len();
  window.navigation().go(Route::Project(
    project,
    ProjectPage::hub(HubView::adding(HubFilter::McpServers)),
  ));
  window.until("the form", |window| window.texts("team-hub-new-create").is_some());
  window.set("team-new-mcp-name", "figma-local");
  window.set("team-new-mcp-program", "npx");
  window.press("team-hub-new-create");
  window.until("the refusal", |window| window.texts("team-new-refused").is_some());
  assert_eq!(
    window.text("team-new-refused-title"),
    "The team's server refused: the name figma-local is taken"
  );
  // The refusal says it is the server's word; once this computer reads the team's list again, which then holds the
  // name, the field and the bar say it as they say any name the team has. Each moment has its own pair of words, read
  // in one frame: a field of one moment with the hint of the other is a defect.
  let pair = |window: &mut Window| {
    let said = |window: &mut Window, id: &str| window.texts(id).map(|lines| lines.join(" "));
    (
      said(window, "team-new-mcp-name-row-sub"),
      said(window, "team-hub-actions-hint"),
    )
  };
  let (told, read_again) = (
    (
      "Taken in the team · from the server",
      "Change the name, then Create again",
    ),
    (
      "The team already has an MCP server named figma-local · pick another name",
      "Fix the name to create it",
    ),
  );
  window.until("the field and the hint to say the same moment", |window| {
    let (field, hint) = pair(window);
    [told, read_again]
      .iter()
      .any(|(want_field, want_hint)| field.as_deref() == Some(want_field) && hint.as_deref() == Some(want_hint))
  });
  assert_eq!(
    db(&window).resources(ResourceKind::McpServer).expect("resources").len(),
    before
  );

  window.set("team-new-mcp-name", "figma-remote");
  window.until("the field to clear", |window| {
    window.text("team-new-mcp-name-row-sub") != "Taken in the team · from the server"
  });
}

#[test]
fn team_qa_hub_edit_automation_does_not_save_what_agents_may_do_unless_a_person_let_it() {
  let (mut window, server, _) = on_pencil(("team-edit", State::TeamEdit));
  typed(&mut window);
  with_synthetic_input(|| window.press(SAVE));
  assert_eq!(
    window.refused_press(),
    Some("publish a new version of a team MCP server"),
    "refused to the client, as its tool call's error"
  );
  for _ in 0..5 {
    window.tree.tick_futures();
    window.pass();
  }
  assert!(
    window.texts("team-hub-edit-refused").is_none(),
    "the panel says nothing of it"
  );
  let kept = window
    .tree
    .get_element_by_id_mut(ARGUMENTS)
    .and_then(|element| element.as_text_input())
    .map(|input| input.value());
  assert_eq!(
    kept.as_deref(),
    Some(
      "--stdio
--app
desktop
--no-telemetry"
    ),
    "what was typed stays"
  );
  assert!(nothing_published(&server), "nothing was sent");

  // Once the person lets automation change agent permissions, the client's Save publishes.
  window
    .launch
    .workspace()
    .expect("the session started")
    .set_automation_changes_permissions(true)
    .expect("the permissions opt-in");
  with_synthetic_input(|| window.press(SAVE));
  assert_eq!(window.refused_press(), None, "taken under the consent");
  window.until("the server to number it", |_| !nothing_published(&server));
}
