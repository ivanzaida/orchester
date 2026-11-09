//! An admin's edit of a team MCP server in the Agents hub, and what the team server answers, against the stand-in team
//! server (`Agents · v2` A13 `WMtxm`, A14 `gTLx6`, A15 `dLqHC`, A16 `x3ywL`, A17 `rJbPm`, ORC-836): *Edit* makes the
//! team's part a form kept as a draft, *Publish for the team* sends the next version and says the answer in place, and
//! a refusal keeps the draft and what was typed. On the QA states `team-edit`, `team-edit-demoted`, `team-edit-offline`
//! and `team-edit-rejects`.
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
  resource::{ResourceKind, TeamMcpServer, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use tempfile::TempDir;
use tokio::runtime::Runtime;

use super::team::{self, State};

pub(super) const ARGUMENTS: &str = "team-new-mcp-arguments";
pub(super) const PUBLISH: &str = "team-hub-edit-publish";

/// The window on the seed's `scenario` joined to the stand-in team server in `state`, on the hub's MCP servers with
/// `pencil-design` open, once its *Edit* shows.
fn on_pencil(of: (&str, State)) -> (Window, TeamServer, (ProjectId, ResourceId)) {
  on_pencil_until(of, "team-hub-edit")
}

/// [`on_pencil`], once the panel's element `shown` has its text.
pub(super) fn on_pencil_until(
  (scenario, state): (&str, State),
  shown: &str,
) -> (Window, TeamServer, (ProjectId, ResourceId)) {
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
  // A server the team archived while its draft is kept is listed apart, which the state of ORC-837's A21 holds.
  let kept = db(&window);
  let pencil = kept
    .resources(ResourceKind::McpServer)
    .expect("resources")
    .into_iter()
    .chain(
      kept
        .archived_with_draft(ResourceKind::McpServer)
        .expect("archived resources")
        .into_iter()
        .map(|(resource, _)| resource),
    )
    .find(|resource| resource.name().as_str() == "pencil-design")
    .expect("the seed's pencil-design")
    .id();
  window.navigation().go(Route::Project(
    seeded.project,
    ProjectPage::hub(HubView::new(HubFilter::McpServers, Some(pencil))),
  ));
  window.until("the panel", |window| window.texts(shown).is_some());
  (window, server, (seeded.project, pencil))
}

pub(super) fn db(window: &Window) -> ProjectDb {
  ProjectDb::open(&window.data.repo("repo-kontur")).expect("database")
}

/// From now on this computer cannot keep the number the team server gives a version published here, as when its disk
/// refuses the write: a send reaches the server, which numbers the version, and then fails here, so the version stays
/// in the outbox, a failure that is not the server being out of reach.
pub(super) fn fail_keeping_the_servers_number(window: &Window) {
  rusqlite::Connection::open(window.data.repo("repo-kontur").join(".orchester/orchester.db"))
    .expect("the project database")
    .execute_batch(
      "CREATE TRIGGER fail_keeping_the_servers_number BEFORE DELETE ON shared_publishes \
       BEGIN SELECT RAISE(ABORT, 'the write failed'); END;",
    )
    .expect("the failing write");
}

/// Edit pressed and the arguments typed: the draft is kept once the panel says so.
fn typed(window: &mut Window) {
  window.press("team-hub-edit");
  window.until("the form", |window| window.texts(PUBLISH).is_some());
  window.set(ARGUMENTS, "--stdio\n--app\ndesktop\n--no-telemetry");
  window.until("the draft to be kept", |window| {
    window
      .texts("team-hub-edit-draft-status")
      .is_some_and(|said| said.join(" ").contains("saved"))
  });
  // What the panel says is the save's answer; the database holds it once that came.
  let pencil = db(window)
    .resources(ResourceKind::McpServer)
    .expect("resources")
    .into_iter()
    .find(|resource| resource.name().as_str() == "pencil-design")
    .expect("the seed's pencil-design")
    .id();
  window.until("the draft in the database", |window| {
    draft_of(window, pencil).is_some_and(|args| args == ["--stdio", "--app", "desktop", "--no-telemetry"])
  });
}

pub(super) fn draft_of(window: &Window, id: ResourceId) -> Option<Vec<String>> {
  db(window).draft::<TeamMcpServer>(id).expect("read").map(|draft| {
    match serde_json::to_value(draft.body()).expect("a body")["launch"]["args"].clone() {
      serde_json::Value::Array(args) => args.iter().filter_map(|arg| arg.as_str().map(str::to_owned)).collect(),
      _ => Vec::new(),
    }
  })
}

#[test]
fn team_qa_hub_edit_keeps_a_draft_and_publishing_makes_the_next_version() {
  let (mut window, server, (_, pencil)) = on_pencil(("team-edit", State::TeamEdit));
  typed(&mut window);
  let typed_arguments = ["--stdio", "--app", "desktop", "--no-telemetry"].map(str::to_owned);
  window.until("the draft to hold what was typed", |window| {
    draft_of(window, pencil).as_deref() == Some(&typed_arguments[..])
  });
  let said = window.text("team-hub-edit-draft-sub");
  assert!(said.starts_with("Arguments changed"), "{said}");
  assert!(said.contains("publishing makes v2"), "{said}");
  assert!(window.texts("team-hub-chip-1").is_some(), "the draft is a chip");

  window.press(PUBLISH);
  window.until("the server to number it", |_| {
    server
      .resources("kontur")
      .iter()
      .any(|(resource, versions)| resource["name"] == "pencil-design" && versions.len() == 2)
  });
  window.until("the panel to read as published", |window| {
    window.texts(PUBLISH).is_none() && window.texts("team-hub-edit").is_some()
  });
  window.until("the team's version", |window| {
    window.text("team-hub-chip-team") == "Team · v2"
  });
  assert!(draft_of(&window, pencil).is_none(), "the draft became the version");
  let held = db(&window)
    .latest_version::<TeamMcpServer>(pencil)
    .expect("read")
    .expect("a version");
  assert_eq!(
    held.number(),
    VersionNumber::new(std::num::NonZeroU32::new(2).expect("2"))
  );
}

#[test]
fn team_qa_hub_edit_discard_goes_back_to_the_version() {
  let (mut window, _server, (_, pencil)) = on_pencil(("team-edit", State::TeamEdit));
  typed(&mut window);
  window.press("team-hub-edit-discard");
  window.until("the plain panel", |window| window.texts(PUBLISH).is_none());
  assert!(draft_of(&window, pencil).is_none());
  window.press("team-hub-edit");
  window.until("the form", |window| window.texts(PUBLISH).is_some());
  window.until("nothing kept", |window| {
    window
      .texts("team-hub-edit-draft-status")
      .is_some_and(|said| said.join(" ").contains("nothing changed yet"))
  });
}

#[test]
fn team_qa_hub_edit_no_longer_an_admin_turns_the_page_read_only_and_keeps_the_draft() {
  let (mut window, server, (_, pencil)) = on_pencil(("team-edit-demoted", State::TeamEditDemoted));
  typed(&mut window);
  window.press(PUBLISH);
  window.until("the refusal", |window| {
    window.texts("team-hub-edit-not-admin").is_some()
  });
  let title = window.text("team-hub-edit-not-admin-title");
  assert_eq!(title, "The team's server refused: you are no longer an admin");
  let body = window.text("team-hub-edit-not-admin-body");
  assert!(
    body.contains("Your draft of pencil-design stays on this computer"),
    "{body}"
  );
  assert!(body.contains("Ask an Admin"), "{body}");
  assert!(window.texts(PUBLISH).is_none(), "nothing to publish with");
  assert!(window.texts(ARGUMENTS).is_none(), "the part is read-only");
  assert!(window.texts("team-hub-edit-copy").is_some());
  assert!(window.text("team-hub-actions-hint").starts_with("Ask an Admin"));
  assert!(draft_of(&window, pencil).is_some(), "the draft stays on this computer");
  assert!(
    server
      .resources("kontur")
      .iter()
      .all(|(_, versions)| versions.len() == 1),
    "nothing was published"
  );

  window.press("team-hub-edit-discard");
  window.until("the draft to go", |window| draft_of(window, pencil).is_none());
}

#[test]
fn team_qa_hub_edit_offline_waits_in_the_outbox_and_can_be_cancelled() {
  let (mut window, server, (_, pencil)) = on_pencil(("team-edit-offline", State::TeamEditOffline));
  typed(&mut window);
  window.press(PUBLISH);
  window.until("the waiting notice", |window| {
    window.texts("team-hub-edit-waiting").is_some()
  });
  let title = window.text("team-hub-edit-waiting-title");
  assert_eq!(title, "Waiting to send · retries when the server is back");
  assert_eq!(
    window.text("team-hub-chip-team"),
    "Team · v1",
    "the team's version is still v1"
  );
  assert!(window.texts("team-hub-edit-cancel").is_some());
  assert!(window.texts(PUBLISH).is_none() && window.texts(ARGUMENTS).is_none());
  assert_eq!(db(&window).waiting_publishes().expect("outbox").len(), 1);
  assert!(
    server
      .resources("kontur")
      .iter()
      .all(|(_, versions)| versions.len() == 1)
  );

  window.press("team-hub-edit-cancel");
  window.until("the draft to come back", |window| window.texts(PUBLISH).is_some());
  assert!(db(&window).waiting_publishes().expect("outbox").is_empty());
  assert!(draft_of(&window, pencil).is_some(), "what was typed is the draft again");
  // The chip reads the hub again after the version went: until then it still names the version that was taken back.
  window.until("the team's version again", |window| {
    window.text("team-hub-chip-team") == "Team · v1"
  });
}

#[test]
fn team_qa_hub_edit_a_rejected_content_says_why_in_place_and_keeps_what_was_typed() {
  let (mut window, _server, (_, pencil)) = on_pencil(("team-edit-rejects", State::TeamEditRejects));
  typed(&mut window);
  window.press(PUBLISH);
  window.until("the refusal", |window| window.texts("team-hub-edit-refused").is_some());
  assert_eq!(
    window.text("team-hub-edit-refused-title"),
    "The team's server refused v2"
  );
  let body = window.text("team-hub-edit-refused-body");
  assert!(body.contains("does not accept its content"), "{body}");
  assert!(body.contains("Your draft is kept"), "{body}");
  assert!(window.text("team-hub-actions-hint").starts_with("Refused "));
  assert!(
    window.texts(PUBLISH).is_some(),
    "the form stays, to fix it and publish again"
  );
  assert!(draft_of(&window, pencil).is_some());
  assert!(db(&window).waiting_publishes().expect("outbox").is_empty());
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
fn team_qa_hub_edit_automation_does_not_publish_what_agents_may_do_unless_a_person_let_it() {
  let (mut window, server, (_, pencil)) = on_pencil(("team-edit", State::TeamEdit));
  typed(&mut window);
  with_synthetic_input(|| window.press(PUBLISH));
  window.until("the refusal", |window| window.texts("team-hub-edit-refused").is_some());
  let body = window.text("team-hub-edit-refused-body");
  assert!(body.contains("Only a person can do this"), "{body}");
  assert!(draft_of(&window, pencil).is_some(), "the draft is kept");
  assert!(
    server
      .resources("kontur")
      .iter()
      .all(|(_, versions)| versions.len() == 1),
    "nothing was sent"
  );
  assert!(db(&window).waiting_publishes().expect("outbox").is_empty());
}
