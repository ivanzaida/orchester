//! The team profile form on the QA states (`Agents · v2` A9, ORC-848 D8): its CLI and model selects offer what this
//! computer's connections run, so a QA copy that joins the stand-in team server shows them filled.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_hub_profile_form`

use orchester_app::routes::{HubFilter, HubView, ProjectPage, Route};

use super::{not_ready::on_task, team::State};

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
  window.until("the form", |window| window.texts("team-new-profile-name").is_some());
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
