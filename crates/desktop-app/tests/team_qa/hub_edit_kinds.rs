//! An admin edits a team instruction, skill and command tool in place in the hub's panel, against the stand-in team
//! server (`Agents · v2` A27 `uVUZ4`, A28 `qfrbn`, A29 `FrraS`, A30 `U5e2m`, A31 `rHN6u`;
//! `docs/design/team-configuration-live.md` rule 6): Save publishes the next version at once; a version a teammate
//! published first is refused, and the panel reloads from it and says who published it; while the team server cannot
//! be reached what was typed stays, Save publishes nothing and Cancel still puts back the version. On the QA state
//! `team-edit`; a member's read-only panels are `--test agents team_hub_member`'s.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_hub_edit_kinds`

use lurq::app::events::with_synthetic_input;
use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  routes::{HubFilter, HubView},
  test_support::Window,
};
use orchester_core::{id::ResourceId, resource::ResourceKind, store::ResourceStore};
use serde_json::{Value, json};

use super::{
  hub_edit::{db, opened, says_unreachable},
  team::State,
};

const SAVE: &str = "team-hub-edit-save";
const CANCEL: &str = "team-hub-edit-cancel";
const TEXT: &str = "team-hub-text-input";

/// The window on `state`, on the team resource `name` of `kind` under `filter`, once the panel's `shown` is there.
fn on(state: (&str, State), (filter, kind, name): (HubFilter, ResourceKind, &str), shown: &str) -> Opened {
  let name = name.to_owned();
  let (window, server, (_, picked)) = opened(state, shown, move |window| {
    let id = db(window)
      .resources(kind)
      .expect("resources")
      .into_iter()
      .find(|resource| resource.name().as_str() == name)
      .expect("the seed's resource")
      .id();
    (HubView::new(filter, Some(id)), Some(id))
  });
  Opened {
    window,
    server,
    id: picked.expect("the resource"),
  }
}

struct Opened {
  window: Window,
  server: TeamServer,
  id: ResourceId,
}

impl Opened {
  /// The versions the stand-in holds of the resource.
  fn versions(&self) -> Vec<Value> {
    self
      .server
      .resources("kontur")
      .into_iter()
      .find(|(resource, _)| resource["id"] == self.id.to_string())
      .map(|(_, versions)| versions)
      .expect("the stand-in holds the resource")
  }

  /// Types `value` into `field` and waits for Save to say what it publishes.
  fn type_into(&mut self, field: &str, value: &str) {
    self.window.set(field, value);
    self.window.until("the panel to hold the change", unsaved);
  }

  /// Presses Save and waits for the stand-in to hold `count` versions and the panel to read the latest.
  fn saved(&mut self, count: usize) {
    self.window.press(SAVE);
    self.published(count);
  }

  /// Waits for the stand-in to hold `count` versions and the panel to read the latest.
  fn published(&mut self, count: usize) {
    let id = self.id;
    let server = &self.server;
    self.window.until("the server to number it", |_| {
      server
        .resources("kontur")
        .into_iter()
        .any(|(resource, versions)| resource["id"] == id.to_string() && versions.len() == count)
    });
    let team = format!("Team · v{count}");
    self
      .window
      .until("the team's version", |window| window.text("team-hub-chip-team") == team);
    assert!(!unsaved(&mut self.window), "nothing differs from what was published");
  }
}

fn instruction(state: (&str, State)) -> Opened {
  on(
    state,
    (HubFilter::Instructions, ResourceKind::Instruction, "Project rules"),
    "team-hub-panel-title",
  )
}

#[test]
fn team_qa_hub_edit_kinds_an_instructions_save_publishes_its_next_version() {
  let mut opened = instruction(("team-edit", State::TeamEdit));
  opened
    .window
    .until("the text to edit", |window| window.texts(TEXT).is_some());
  opened.type_into(TEXT, "Keep each commit small.\n\nRun the tests before you hand off.");
  assert!(
    opened.window.texts("team-hub-text-changed").is_some(),
    "the text says it changed"
  );
  opened.saved(2);
  let text = &opened.versions()[1]["body"]["text"];
  assert_eq!(
    text,
    &json!("Keep each commit small.\n\nRun the tests before you hand off.")
  );
}

/// An instruction's text widens nothing agents may do, so an MCP client's Save publishes it too.
#[test]
fn team_qa_hub_edit_kinds_automation_saves_an_instructions_text() {
  let mut opened = instruction(("team-edit", State::TeamEdit));
  opened
    .window
    .until("the text to edit", |window| window.texts(TEXT).is_some());
  opened.type_into(TEXT, "Keep each commit small.");
  with_synthetic_input(|| opened.window.press(SAVE));
  opened.published(2);
  assert!(opened.window.texts("team-hub-edit-refused").is_none());
}

#[test]
fn team_qa_hub_edit_kinds_a_skills_save_publishes_its_next_version() {
  let mut opened = on(
    ("team-edit", State::TeamEdit),
    (HubFilter::Skills, ResourceKind::Skill, "review-diff"),
    "team-hub-skill-md-input",
  );
  opened.type_into(
    "team-hub-skill-md-input",
    "# review-diff\n\nEnd with Approve or Changes needed.",
  );
  opened.saved(2);
  let body = &opened.versions()[1]["body"]["body"];
  assert_eq!(body, &json!("# review-diff\n\nEnd with Approve or Changes needed."));
}

#[test]
fn team_qa_hub_edit_kinds_a_command_tools_save_publishes_its_next_version() {
  let mut opened = on(
    ("team-edit", State::TeamEdit),
    (HubFilter::Tools, ResourceKind::Tool, "cargo-check"),
    "team-hub-from-team-arguments-input",
  );
  opened.type_into("team-hub-from-team-arguments-input", "check\n--workspace");
  opened.saved(2);
  let args = &opened.versions()[1]["body"]["action"]["command"]["args"];
  assert_eq!(args, &json!(["check", "--workspace"]));
}

#[test]
fn team_qa_hub_edit_kinds_a_newer_version_refuses_the_save_and_reloads_the_panel() {
  let mut opened = instruction(("team-edit", State::TeamEdit));
  opened
    .window
    .until("the text to edit", |window| window.texts(TEXT).is_some());
  let mut newer = opened.versions()[0]["body"].clone();
  newer["text"] = json!("Ask before you change the database schema.");
  opened.server.publish_as("kontur", opened.id, newer, "olha");
  opened.type_into(TEXT, "Mine, typed on v1.");

  opened.window.press(SAVE);
  opened
    .window
    .until("the refusal", |window| window.texts("team-hub-edit-refused").is_some());
  let title = opened.window.text("team-hub-edit-refused-title");
  assert!(title.contains("published v2 while you edited"), "{title}");
  assert_eq!(
    opened.window.text(TEXT),
    "Ask before you change the database schema.",
    "the panel shows the newer version"
  );
  assert!(!unsaved(&mut opened.window), "nothing differs now");
  assert_eq!(opened.versions().len(), 2, "nothing of this computer's was published");
}

#[test]
fn team_qa_hub_edit_kinds_unreachable_keeps_what_was_typed_and_cancel_puts_back_the_version() {
  let mut opened = instruction(("team-edit", State::TeamEdit));
  opened
    .window
    .until("the text to edit", |window| window.texts(TEXT).is_some());
  let published = opened.window.text(TEXT);
  opened.type_into(TEXT, "Typed while the team server is lost.");
  opened.server.set_down(true);
  opened
    .window
    .until("the hub to say it cannot reach the team", says_unreachable);
  opened.window.until("the bar to wait for the server", |window| {
    window
      .texts("team-hub-actions-hint")
      .is_some_and(|said| said.join(" ").contains("Save needs the team server"))
  });
  assert!(
    opened.window.texts("team-hub-edit-unreachable").is_some(),
    "A31's notice"
  );
  opened.window.press(SAVE);
  assert_eq!(opened.window.text(TEXT), "Typed while the team server is lost.");
  assert_eq!(opened.versions().len(), 1, "nothing was published");

  opened.window.press(CANCEL);
  opened
    .window
    .until("the version back", |window| window.text(TEXT) == published);
}

#[test]
fn team_qa_hub_edit_kinds_a_profiles_role_and_what_it_gives_are_published_as_its_next_version() {
  let mut opened = on(
    ("team-edit", State::TeamEdit),
    (HubFilter::Profiles, ResourceKind::AgentProfile, "Reviewer"),
    "team-hub-role-instructions-input",
  );
  assert!(
    opened.window.texts("team-hub-edit").is_none(),
    "no Edit: the panel edits in place"
  );
  let rules = db(&opened.window)
    .resources(ResourceKind::Instruction)
    .expect("resources")
    .into_iter()
    .find(|resource| resource.name().as_str() == "Project rules")
    .expect("the seed's Project rules")
    .id();
  // Tall enough that the menu under the profile's runs-on selects is in the window, where a pointer presses it.
  opened.window.tree.resize(1440, 2400);
  opened.window.pass();
  opened.window.press("local-hub-profile-add-instructions-button");
  opened.window.press(&format!("local-hub-profile-instructions-{rules}"));
  opened.type_into(
    "team-hub-role-instructions-input",
    "You are the Reviewer. Read the task, then the diff.",
  );
  opened.saved(2);
  let body = &opened.versions()[1]["body"];
  assert_eq!(
    body["instructions"],
    json!("You are the Reviewer. Read the task, then the diff.")
  );
  assert_eq!(body["instruction_resources"], json!([rules.to_string()]));
}

/// Whether the panel's chips say it holds changes that are not saved.
fn unsaved(window: &mut Window) -> bool {
  window
    .texts("team-hub-panel")
    .is_some_and(|said| said.iter().any(|words| words == "Unsaved changes"))
}
