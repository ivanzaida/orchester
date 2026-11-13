//! The team profile form on the QA states (`Agents · v2` A9, ORC-848 D8; `Agents on accounts` C4 `IgFWn`): its one
//! field *CLI, model and effort* opens the *Runs on* picker over what this computer's connections run (the CLIs on the
//! rail, the models of the one shown), so a QA copy that joins the stand-in team server shows them; it makes a profile
//! with its role instructions and the team's resources it gives, and it has the fields an admin's edit has (A7, A26).
//!
//! `cargo test -p orchester-app --test team_qa team_qa_hub_profile_form`

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::routes::{HubFilter, HubView, ProjectPage, Route};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::ResourceKind,
  store::ResourceStore,
};
use serde_json::json;

use super::{hub_edit::db, not_ready::on_task, team::State};

/// The field of what a new profile runs with, and the same field of an admin's edit.
const NEW: &str = "team-new-profile-runs-on";
const EDIT: &str = "team-hub-from-team-runs-on";

/// The rail's item of the CLI `cli` (as the code names the kind) in the picker of the field `field`.
fn on_rail(field: &str, cli: &str) -> String {
  format!("{field}-menu-connection-cli-{cli}")
}

/// Opens the picker of `field`.
fn open(window: &mut orchester_app::test_support::Window, field: &str) {
  let menu = format!("{field}-menu");
  if window.texts(&menu).is_none() {
    window.press(field);
    window.until("the picker", |window| window.texts(&menu).is_some());
  }
}

/// The models the open picker of `field` lists.
fn models(window: &mut orchester_app::test_support::Window, field: &str) -> Vec<String> {
  (0..8)
    .filter_map(|at| window.texts(&format!("{field}-menu-model-{at}")))
    .map(|texts| texts.join(" "))
    .collect()
}

/// Shows the CLI `cli` in the picker of `field` and chooses its model `model`, as a person's presses would.
fn choose(window: &mut orchester_app::test_support::Window, field: &str, (cli, model): (&str, &str)) {
  open(window, field);
  // The rail has the CLIs once this computer's connections are read.
  let item = on_rail(field, cli);
  window.until("the CLI on the rail", |window| window.texts(&item).is_some());
  window.press(&item);
  window.until("the model in the list", |window| {
    models(window, field).iter().any(|listed| listed.contains(model))
  });
  let at = models(window, field)
    .iter()
    .position(|listed| listed.contains(model))
    .expect("listed");
  window.press(&format!("{field}-menu-model-{at}"));
  window.until("the field says it", |window| window.text(field).contains(model));
}

#[test]
fn team_qa_hub_profile_form_offers_the_cli_kinds_and_models_of_the_seeds_connections() {
  let (mut window, _server, project) = on_task(("team-edit", State::TeamEdit), "KON-1");
  window.navigation().go(Route::Project(
    project,
    ProjectPage::hub(HubView::adding(HubFilter::Profiles)),
  ));
  window.until("the form", |window| {
    window.texts("team-new-profile-role-name").is_some()
  });
  assert_eq!(
    window.text("team-new-profile-runs-on-row-label"),
    "CLI, model and effort"
  );
  assert_eq!(window.text(NEW), "Choose a CLI and a model");
  open(&mut window, NEW);
  window.until("the CLI kinds", |window| {
    ["ClaudeCodeCli", "CodexCli"]
      .iter()
      .all(|cli| window.texts(&on_rail(NEW, cli)).is_some())
  });
  window.press(&on_rail(NEW, "CodexCli"));
  window.until("the models", |window| !models(window, NEW).is_empty());
  let listed = models(&mut window, NEW);
  assert!(
    listed.iter().any(|model| model.contains("gpt-5.5")),
    "models: {listed:?}"
  );
  assert_eq!(
    window.text(NEW),
    "Choose a CLI and a model",
    "showing a CLI chooses nothing"
  );
}

/// The admin's window on the team's profile form, on the stand-in team server that holds the seed's resources.
fn form() -> (orchester_app::test_support::Window, TeamServer, ProjectId) {
  let (mut window, server, project) = on_task(("team-edit", State::TeamEdit), "KON-1");
  window.navigation().go(Route::Project(
    project,
    ProjectPage::hub(HubView::adding(HubFilter::Profiles)),
  ));
  window.until("the form", |window| {
    window.texts("team-new-profile-role-name").is_some()
  });
  window.until("the field", |window| window.texts(NEW).is_some());
  (window, server, project)
}

/// The id of the seed's team resource of `kind` named `name`.
fn seeded(window: &orchester_app::test_support::Window, kind: ResourceKind, name: &str) -> ResourceId {
  db(window)
    .resources(kind)
    .expect("resources")
    .into_iter()
    .find(|resource| resource.name().as_str() == name)
    .unwrap_or_else(|| panic!("the seed's {name}"))
    .id()
}

#[test]
fn team_qa_hub_profile_form_creates_a_profile_with_its_role_instructions_and_what_it_gives() {
  let (mut window, server, _) = form();
  let rules = seeded(&window, ResourceKind::Instruction, "Project rules");
  window.set("team-new-profile-role-name-input", "Auditor");
  window.set(
    "team-new-profile-role-instructions-input",
    "You are the Auditor. Read the diff.",
  );
  choose(&mut window, NEW, ("CodexCli", "gpt-5.5"));
  assert_eq!(window.text(NEW), "Codex · gpt-5.5");
  // The picker stays open over the form until it is closed, as a menu does.
  window.press(NEW);
  window.press("local-hub-profile-add-instructions-button");
  window.press(&format!("local-hub-profile-instructions-{rules}"));
  window.until("the instruction's tag", |window| {
    window.texts("team-new-profile-instructions-value-0").is_some()
  });
  window.press("team-hub-new-create");
  window.until("the server to take it", |_| {
    server
      .resources("kontur")
      .iter()
      .any(|(resource, versions)| resource["name"] == "Auditor" && versions.len() == 1)
  });
  let held = server.resources("kontur");
  let (_, versions) = held
    .iter()
    .find(|(resource, _)| resource["name"] == "Auditor")
    .expect("the profile");
  let body = &versions[0]["body"];
  assert_eq!(body["instructions"], json!("You are the Auditor. Read the diff."));
  assert_eq!(body["instruction_resources"], json!([rules.to_string()]));
  assert_eq!(body["model"], json!("gpt-5.5"));
}

/// What each part of the form says: its rows' labels in order, `<prefix>`'s for the role and `<rows>`'s for the rest.
fn labels_of(window: &mut orchester_app::test_support::Window, (role, rows): (&str, &str)) -> Vec<String> {
  let role_parts = ["name", "responsibilities", "instructions"].map(|part| format!("{role}-{part}-label"));
  let selects = [format!("{rows}-runs-on-row-label")];
  let lists = ["instructions", "mcp", "tools", "skills"].map(|part| format!("{rows}-{part}-label"));
  role_parts
    .iter()
    .chain(&selects)
    .chain(&lists)
    .map(|id| window.texts(id).unwrap_or_else(|| panic!("{id} is drawn")).join(" "))
    .collect()
}

#[test]
fn team_qa_hub_profile_form_create_and_edit_draw_the_same_fields() {
  let (mut window, _server, project) = form();
  let created = labels_of(&mut window, ("team-new-profile-role", "team-new-profile"));
  let reviewer = seeded(&window, ResourceKind::AgentProfile, "Reviewer");
  window.navigation().go(Route::Project(
    project,
    ProjectPage::hub(HubView::new(HubFilter::Profiles, Some(reviewer))),
  ));
  window.until("the edit", |window| {
    window.texts("team-hub-role-instructions-input").is_some() && window.texts(EDIT).is_some()
  });
  let edited = labels_of(&mut window, ("team-hub-role", "team-hub-from-team"));
  assert_eq!(created, edited, "one form: the same fields in the same order");
  open(&mut window, EDIT);
  assert!(
    !models(&mut window, EDIT).is_empty(),
    "the edit's field opens the create form's picker, on the version's CLI"
  );
}
