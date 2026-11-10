//! The team profile form on the QA states (`Agents · v2` A9, ORC-848 D8): its CLI and model selects offer what this
//! computer's connections run, so a QA copy that joins the stand-in team server shows them filled; it makes a profile
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

fn labels(window: &mut orchester_app::test_support::Window, id: &str) -> Vec<String> {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_select())
    .map(|select| select.labels().iter().map(ToString::to_string).collect())
    .unwrap_or_default()
}

fn choose(window: &mut orchester_app::test_support::Window, id: &str, label: &str) {
  let mut select = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_select())
    .unwrap_or_else(|| panic!("{id} is a select"));
  let index = select
    .labels()
    .iter()
    .position(|option| &**option == label)
    .unwrap_or_else(|| panic!("an option {label}"));
  select.commit(index);
  window.pass();
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
  window.until("the CLI kinds", |window| {
    !labels(window, "team-new-profile-cli").is_empty()
  });
  let clis = labels(&mut window, "team-new-profile-cli");
  assert!(clis.iter().any(|cli| cli == "Claude Code"), "CLI kinds: {clis:?}");
  assert!(clis.iter().any(|cli| cli == "Codex"), "CLI kinds: {clis:?}");
  choose(&mut window, "team-new-profile-cli", "Codex");
  window.until("the models", |window| {
    !labels(window, "team-new-profile-model").is_empty()
  });
  let models = labels(&mut window, "team-new-profile-model");
  assert!(models.iter().any(|model| model == "gpt-5.5"), "models: {models:?}");
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
  window.until("the CLI kinds", |window| {
    !labels(window, "team-new-profile-cli").is_empty()
  });
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
  choose(&mut window, "team-new-profile-cli", "Codex");
  window.until("the models", |window| {
    !labels(window, "team-new-profile-model").is_empty()
  });
  choose(&mut window, "team-new-profile-model", "gpt-5.5");
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
  let selects = ["cli", "model", "effort"].map(|part| format!("{rows}-{part}-row-label"));
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
    window.texts("team-hub-role-instructions-input").is_some() && window.texts("team-hub-from-team-cli").is_some()
  });
  let edited = labels_of(&mut window, ("team-hub-role", "team-hub-from-team"));
  assert_eq!(created, edited, "one form: the same fields in the same order");
  assert!(
    window
      .tree
      .get_element_by_id_mut("team-hub-from-team-model")
      .and_then(|element| element.as_select())
      .is_some(),
    "the edit's model is the create form's select"
  );
}
