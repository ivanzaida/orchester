//! What the hub's panel of a team MCP server says when the team changed it or archived it while an admin edited it
//! (rule 18 of `docs/design/team-configuration.md`, A18 `bk4i9`, A19 `DV8OI`, A20 `CAPhQ`, A21 `yqYj9`, ORC-837): one
//! quiet line when a newer version came, a refused publish that compares the version the draft was edited from, the
//! team's and the draft field by field, *Rebase my draft* (which keeps what only the person changed, takes the team's
//! for the rest and asks where both changed), *Open vN*, *Discard my draft* in a modal, and *Restore* for a server that
//! was archived meanwhile. On the QA states `team-edit-newer`, `team-edit-conflicting`, `team-edit-conflict-clean` and
//! `team-edit-archived`, made without a press.
//!
//! `cargo test -p orchester-app --features test-support --test team_qa team_qa_hub_conflict`

use lurq::app::events::with_synthetic_input;
use orchester_api::test_support::team_server::TeamServer;
use orchester_app::test_support::Window;
use orchester_core::{
  id::ResourceId,
  resource::{TeamMcpServer, VersionNumber},
  store::ResourceStore,
};
use serde_json::Value;

use super::{
  hub_edit::{ARGUMENTS, PUBLISH, db, on_pencil_until},
  team::State,
};

const NEWER_TEXT: &str = "team-hub-newer-text";
const COMPARE: &str = "team-hub-compare";
const REBASE: &str = "team-hub-conflict-rebase";
const OPEN: &str = "team-hub-conflict-open";
const DISCARD: &str = "team-hub-conflict-discard";
const DISCARD_DIALOG: &str = "team-hub-conflict-discard-dialog";
const FAILURE: &str = "team-hub-edit-action-error";

fn v2() -> VersionNumber {
  VersionNumber::FIRST.next().expect("a second version")
}

/// The draft of `pencil` as it is kept: its body's JSON and the version it is edited from.
fn draft(window: &Window, pencil: ResourceId) -> Option<(Value, VersionNumber)> {
  db(window).draft::<TeamMcpServer>(pencil).expect("read").map(|draft| {
    (
      serde_json::to_value(draft.body()).expect("a body"),
      draft.based_on().expect("a base"),
    )
  })
}

fn arguments(body: &Value) -> Vec<String> {
  body["launch"]["args"]
    .as_array()
    .expect("arguments")
    .iter()
    .filter_map(|argument| argument.as_str().map(str::to_owned))
    .collect()
}

fn exempt(body: &Value) -> Vec<String> {
  body["exempt_tools"]
    .as_array()
    .expect("tools")
    .iter()
    .filter_map(|tool| tool.as_str().map(str::to_owned))
    .collect()
}

fn on(scenario: &str, state: State, shown: &str) -> (Window, TeamServer, ResourceId) {
  let (window, server, (_, pencil)) = on_pencil_until((scenario, state), shown);
  (window, server, pencil)
}

#[test]
fn team_qa_hub_conflict_a_newer_version_says_so_in_one_line_and_changes_nothing_in_the_draft() {
  let (mut window, server, pencil) = on("team-edit-newer", State::TeamEditNewer, NEWER_TEXT);
  let said = window.text(NEWER_TEXT);
  assert!(
    said.starts_with("olha published v2 at ") && said.ends_with("· your draft is based on v1"),
    "{said}"
  );
  assert!(window.texts("team-hub-newer-view").is_some(), "View changes");
  let line = window.text(&format!("team-hub-row-{pencil}-line"));
  assert!(line.starts_with("Your draft on v1 · v2 is out"), "{line}");
  assert!(
    window.texts(PUBLISH).is_some() && window.texts(ARGUMENTS).is_some(),
    "the form is still there"
  );
  assert!(window.texts(COMPARE).is_none(), "the comparison waits for View changes");
  let (kept, based_on) = draft(&window, pencil).expect("the draft is kept");
  assert_eq!(
    based_on,
    VersionNumber::FIRST,
    "the draft still says it was edited from v1"
  );
  assert_eq!(arguments(&kept).last().map(String::as_str), Some("3845"));
  assert_eq!(
    server
      .resources("kontur")
      .iter()
      .map(|(_, versions)| versions.len())
      .max(),
    Some(2),
    "nothing was sent"
  );
}

#[test]
fn team_qa_hub_conflict_view_changes_compares_without_refusing_anything() {
  let (mut window, _server, pencil) = on("team-edit-newer", State::TeamEditNewer, NEWER_TEXT);
  window.press("team-hub-newer-view");
  window.until("the comparison", |window| window.texts(COMPARE).is_some());
  assert!(
    window.texts("team-hub-conflict-notice").is_none(),
    "no publish was refused"
  );
  assert!(window.texts(REBASE).is_some() && window.texts(OPEN).is_some() && window.texts(DISCARD).is_some());
  assert_eq!(
    window.text("team-hub-compare-counts"),
    "· 1 both changed · 1 theirs · 1 yours"
  );
  assert!(draft(&window, pencil).is_some());
}

#[test]
fn team_qa_hub_conflict_publishing_on_an_older_version_is_refused_as_a_conflict_with_the_comparison() {
  let (mut window, server, pencil) = on("team-edit-newer", State::TeamEditNewer, NEWER_TEXT);
  window.press(PUBLISH);
  window.until("the refusal", |window| {
    window.texts("team-hub-conflict-notice").is_some()
  });
  let title = window.text("team-hub-conflict-notice-title");
  assert_eq!(title, "The team's server refused: v2 was published while you edited");
  assert_eq!(window.text("team-hub-chip-here"), "Conflict · 1 field both changed");
  assert!(window.texts(COMPARE).is_some());
  assert!(window.text("team-hub-chips-end").starts_with("refused "));
  assert_eq!(
    draft(&window, pencil).map(|(_, based_on)| based_on),
    Some(VersionNumber::FIRST),
    "the draft is kept, on the version it was edited from"
  );
  assert!(db(&window).waiting_publishes().expect("outbox").is_empty());
  assert_eq!(
    server
      .resources("kontur")
      .iter()
      .map(|(_, versions)| versions.len())
      .max(),
    Some(2),
    "nothing was sent: this computer already knew"
  );
}

#[test]
fn team_qa_hub_conflict_the_comparison_says_what_each_side_changed_field_by_field() {
  let (mut window, _server, pencil) = on("team-edit-conflicting", State::TeamEditConflicting, COMPARE);
  assert_eq!(window.text("team-hub-compare-arguments-result"), "Both changed · pick");
  assert_eq!(window.text("team-hub-compare-exempt-result"), "Takes theirs");
  assert_eq!(window.text("team-hub-compare-variables-result"), "Keeps yours");
  assert_eq!(window.text("team-hub-compare-folder-result"), "Same in both");
  assert_eq!(
    window.text("team-hub-compare-arguments-base"),
    "v1: --stdio --app desktop"
  );
  assert_eq!(
    window.text("team-hub-compare-arguments-theirs"),
    "--stdio --app desktop --port 3846"
  );
  assert_eq!(
    window.text("team-hub-compare-arguments-mine"),
    "--stdio --app desktop --port 3845"
  );
  assert_eq!(window.text("team-hub-chip-here"), "Conflict · 1 field both changed");
  let line = window.text(&format!("team-hub-row-{pencil}-line"));
  assert!(line.starts_with("Conflict with v2 · publish refused"), "{line}");
  assert_eq!(
    db(&window).waiting_publishes().expect("outbox").len(),
    1,
    "it met v2 in the outbox"
  );
  assert!(window.texts(PUBLISH).is_none(), "no publish on top of it");
}

#[test]
fn team_qa_hub_conflict_rebase_asks_per_field_where_both_changed_and_carries_the_rest() {
  let (mut window, _server, pencil) = on("team-edit-conflicting", State::TeamEditConflicting, COMPARE);
  assert!(
    window
      .text("team-hub-actions-hint")
      .starts_with("Pick Mine or Theirs for Arguments"),
    "rebase asks first"
  );
  window.press(REBASE);
  assert!(
    db(&window).waiting_publishes().expect("outbox").len() == 1,
    "a rebase with an open question changes nothing"
  );
  window.press("team-hub-compare-arguments-pick-theirs");
  window.until("rebase to be ready", |window| {
    window.text("team-hub-actions-hint") == "Rebase gives a draft on v2 to publish"
  });
  window.press(REBASE);
  window.until("the draft on v2", |window| {
    draft(window, pencil).is_some_and(|(_, based_on)| based_on == v2())
  });
  let (kept, _) = draft(&window, pencil).expect("a draft");
  assert_eq!(
    arguments(&kept),
    ["--stdio", "--app", "desktop", "--port", "3846"],
    "theirs, picked"
  );
  assert!(
    exempt(&kept).contains(&"get_images".to_owned()),
    "theirs, which the person left alone"
  );
  assert_eq!(
    kept["launch"]["variables"],
    serde_json::json!(["PENCIL_TOKEN"]),
    "mine, which they left alone"
  );
  assert_eq!(
    kept["launch"]["working_directory"], "tools/pencil",
    "what both changed the same way"
  );
  assert!(
    db(&window).waiting_publishes().expect("outbox").is_empty(),
    "taken back as the draft"
  );
  window.until("the form", |window| window.texts(PUBLISH).is_some());
  assert!(window.texts(COMPARE).is_none());
}

#[test]
fn team_qa_hub_conflict_rebase_picking_mine_keeps_the_persons_field() {
  let (mut window, _server, pencil) = on("team-edit-conflicting", State::TeamEditConflicting, COMPARE);
  window.press("team-hub-compare-arguments-pick-mine");
  window.until("rebase to be ready", |window| {
    window.text("team-hub-actions-hint") == "Rebase gives a draft on v2 to publish"
  });
  window.press(REBASE);
  window.until("the draft on v2", |window| {
    draft(window, pencil).is_some_and(|(_, based_on)| based_on == v2())
  });
  let (kept, _) = draft(&window, pencil).expect("a draft");
  assert_eq!(arguments(&kept), ["--stdio", "--app", "desktop", "--port", "3845"]);
}

#[test]
fn team_qa_hub_conflict_rebase_needs_no_pick_when_no_field_was_changed_by_both() {
  let (mut window, _server, pencil) = on("team-edit-conflict-clean", State::TeamEditConflictClean, COMPARE);
  assert_eq!(
    window.text("team-hub-actions-hint"),
    "Rebase gives a draft on v2 to publish"
  );
  window.press(REBASE);
  window.until("the draft on v2", |window| {
    draft(window, pencil).is_some_and(|(_, based_on)| based_on == v2())
  });
  let (kept, _) = draft(&window, pencil).expect("a draft");
  assert_eq!(
    arguments(&kept),
    ["--stdio", "--app", "desktop", "--port", "3845"],
    "mine"
  );
  assert!(exempt(&kept).contains(&"get_images".to_owned()), "theirs");
}

#[test]
fn team_qa_hub_conflict_open_the_new_version_keeps_the_draft_and_shows_the_team_s_version() {
  let (mut window, _server, pencil) = on("team-edit-conflicting", State::TeamEditConflicting, COMPARE);
  window.press(OPEN);
  window.until("the team's version", |window| window.texts("team-hub-edit").is_some());
  assert!(window.texts(COMPARE).is_none() && window.texts(PUBLISH).is_none());
  assert!(
    db(&window).waiting_publishes().expect("outbox").is_empty(),
    "taken back as the draft"
  );
  assert_eq!(
    draft(&window, pencil).map(|(_, based_on)| based_on),
    Some(VersionNumber::FIRST),
    "the draft is kept"
  );
}

#[test]
fn team_qa_hub_conflict_discard_asks_first_and_then_throws_the_draft_away() {
  let (mut window, _server, pencil) = on("team-edit-conflicting", State::TeamEditConflicting, COMPARE);
  window.press(DISCARD);
  window.until("the modal", |window| window.texts(DISCARD_DIALOG).is_some());
  assert_eq!(
    window.text(&format!("{DISCARD_DIALOG}-title")),
    "Discard your draft of pencil-design?"
  );
  let body = window.text("team-hub-conflict-discard-body");
  assert!(body.contains("Arguments and Variables it needs"), "{body}");
  assert!(body.contains("v2 stays as olha published it"), "{body}");
  window.press(&format!("{DISCARD_DIALOG}-cancel"));
  window.until("the modal to close", |window| window.texts(DISCARD_DIALOG).is_none());
  assert!(
    draft(&window, pencil).is_none(),
    "the version of the outbox is still there"
  );
  assert_eq!(
    db(&window).waiting_publishes().expect("outbox").len(),
    1,
    "keeping it changed nothing"
  );

  window.press(DISCARD);
  window.until("the modal", |window| window.texts(DISCARD_DIALOG).is_some());
  window.press("team-hub-conflict-discard-confirm");
  window.until("the draft to go", |window| {
    db(window).waiting_publishes().expect("outbox").is_empty() && draft(window, pencil).is_none()
  });
  window.until("the team's version", |window| window.texts("team-hub-edit").is_some());
}

#[test]
fn team_qa_hub_conflict_automation_does_not_rebase_discard_or_restore() {
  let (mut window, _server, pencil) = on("team-edit-conflict-clean", State::TeamEditConflictClean, COMPARE);
  with_synthetic_input(|| window.press(REBASE));
  window.until("the refusal", |window| window.texts(FAILURE).is_some());
  assert!(window.text(FAILURE).contains("Only a person can do this"));
  assert_eq!(
    db(&window).waiting_publishes().expect("outbox").len(),
    1,
    "nothing changed"
  );

  // Opening the question is allowed; answering it is a person's.
  with_synthetic_input(|| window.press(DISCARD));
  window.until("the modal", |window| window.texts(DISCARD_DIALOG).is_some());
  with_synthetic_input(|| window.press("team-hub-conflict-discard-confirm"));
  window.until("the modal to close", |window| window.texts(DISCARD_DIALOG).is_none());
  assert_eq!(
    db(&window).waiting_publishes().expect("outbox").len(),
    1,
    "the draft is kept"
  );
  assert!(draft(&window, pencil).is_none());

  let (mut window, _server, pencil) = on(
    "team-edit-archived",
    State::TeamEditArchived,
    "team-hub-archived-notice",
  );
  with_synthetic_input(|| window.press("team-hub-archived-restore"));
  window.until("the modal", |window| window.texts("team-hub-restore-dialog").is_some());
  with_synthetic_input(|| window.press("team-hub-restore-confirm"));
  window.until("the refusal", |window| window.texts(FAILURE).is_some());
  assert!(
    db(&window).resource_removed_at(pencil).expect("read").is_some(),
    "it stays archived"
  );
}

#[test]
fn team_qa_hub_conflict_an_archived_server_keeps_the_draft_and_restore_brings_it_back() {
  let (mut window, server, pencil) = on(
    "team-edit-archived",
    State::TeamEditArchived,
    "team-hub-archived-notice",
  );
  let title = window.text("team-hub-archived-notice-title");
  assert!(
    title.starts_with("The team's server refused: pencil-design was archived at "),
    "{title}"
  );
  assert_eq!(window.text("team-hub-chip-here"), "Archived");
  assert_eq!(window.text("team-hub-chip-1"), "Your draft on v1");
  let head = window.text("team-hub-panel-version");
  assert!(
    head.starts_with("v1 · archived ") && !head.contains("published by"),
    "the head says when it was archived, not who published it: {head}"
  );
  assert!(
    window.texts("team-hub-section-archived").is_some(),
    "listed apart, under Archived"
  );
  let line = window.text(&format!("team-hub-row-{pencil}-line"));
  assert!(line.ends_with("· your draft is kept"), "{line}");
  assert!(draft(&window, pencil).is_some(), "the draft is kept");
  assert!(
    window.texts(PUBLISH).is_none(),
    "nothing is published while it is archived"
  );

  window.press("team-hub-archived-restore");
  window.until("the question", |window| {
    window.texts("team-hub-restore-dialog").is_some()
  });
  assert_eq!(window.text("team-hub-restore-dialog-title"), "Restore pencil-design?");
  assert!(draft(&window, pencil).is_some() && db(&window).resource_removed_at(pencil).expect("read").is_some());
  window.press("team-hub-restore-confirm");
  window.until("it to be back", |window| {
    db(window).resource_removed_at(pencil).expect("read").is_none()
  });
  assert!(
    server
      .resources("kontur")
      .iter()
      .any(|(resource, _)| resource["name"] == "pencil-design" && resource["archived"] == false),
    "the server restored it for everyone"
  );
  window.until("the draft as a form", |window| window.texts(PUBLISH).is_some());
  assert!(draft(&window, pencil).is_some(), "the draft is kept");
}
