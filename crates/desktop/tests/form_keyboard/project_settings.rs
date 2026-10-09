//! Project settings (02.N General, 02.M Limits, Environment) and the linked repository in New task. Only the
//! settings form's buttons take Tab stops; the tabs are pointer-only. Buttons that run `gh` or
//! `git` are not pressed: the tests never run the real programs.

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  limits::ProjectLimits,
  resource::{CommandEnvironment, VariableName},
  store::SettingsStore,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::{
  github::{ReplayGh, SIGNED_IN, VERSION, contains_text, github_connection, link, shows},
  harness::{Desktop, expected},
};

const REPOSITORY: &str = "settings-repository";
const CHECK: &str = "settings-check-repository";
const USE_SIGNED_IN: &str = "settings-use-signed-in";
const DETECT: &str = "settings-detect-repository";
/// Repository agent files (ORC-15): its switch and Import from repository close the form.
const AGENT_FILES: &str = "settings-agent-files";
const IMPORT: &str = "settings-agent-files-import";

fn settings(desktop: &mut Desktop) {
  desktop.wait_for("nav-settings");
  desktop.click("nav-settings");
  desktop.wait_for(REPOSITORY);
  desktop.wait_for(AGENT_FILES);
}

/// gh is signed in as ivanzaida; the project expects `expected`.
fn linked(expected: &str) -> Desktop {
  let desktop = Desktop::with_project();
  let github = github_connection(&desktop, &ReplayGh::new(Ok(VERSION), Some(SIGNED_IN)));
  link(&desktop, github, expected);
  desktop
}

#[test]
fn project_settings_show_the_linked_repository_and_the_wrong_account() {
  let mut desktop = linked("ivan-work");
  settings(&mut desktop);
  assert!(shows(&mut desktop, REPOSITORY, "ivanzaida / kontur"));
  assert!(shows(
    &mut desktop,
    "settings-general",
    "github.com · origin · default branch main"
  ));
  assert!(shows(&mut desktop, "settings-expected-account", "ivan-work"));
  assert!(shows(
    &mut desktop,
    "settings-general",
    "gh is signed in as ivanzaida · pull requests blocked"
  ));
  assert!(shows(&mut desktop, USE_SIGNED_IN, "Use signed-in account (ivanzaida)"));
  assert!(desktop.tree.get_element_by_id("settings-expected-notice").is_some());
  // The settings form: Check again, Use signed-in account and the repository agent files; the
  // tabs and the sidebar are pointer-only.
  assert_eq!(
    desktop.stops(5, false),
    expected(&[
      Some(CHECK),
      Some(USE_SIGNED_IN),
      Some(AGENT_FILES),
      Some(IMPORT),
      Some(CHECK)
    ])
  );
}

#[test]
fn project_settings_offer_use_signed_in_only_for_another_account() {
  let mut desktop = linked("ivanzaida");
  settings(&mut desktop);
  assert!(shows(&mut desktop, "settings-general", "gh is signed in as ivanzaida"));
  assert!(desktop.tree.get_element_by_id(USE_SIGNED_IN).is_none());
  // The notice explains Use signed-in account and goes with it.
  assert!(desktop.tree.get_element_by_id("settings-expected-notice").is_none());
  assert_eq!(
    desktop.stops(4, false),
    expected(&[Some(CHECK), Some(AGENT_FILES), Some(IMPORT), Some(CHECK)])
  );
}

#[test]
fn project_settings_without_a_link_offer_detection() {
  let mut desktop = Desktop::with_project();
  settings(&mut desktop);
  assert!(shows(&mut desktop, REPOSITORY, "Not linked"));
  assert!(desktop.tree.get_element_by_id("settings-expected-account").is_none());
  assert!(desktop.tree.get_element_by_id("github-detection").is_none());
  assert_eq!(
    desktop.stops(4, false),
    expected(&[Some(DETECT), Some(AGENT_FILES), Some(IMPORT), Some(DETECT)])
  );
}

pub(crate) const DEFAULT_BUDGET: &str = "limits-default-run-budget";
pub(crate) const DAILY_BUDGET: &str = "limits-daily-budget";
const TIME_LIMIT: &str = "limits-default-time-limit";
pub(crate) const AGENT_SLOTS: &str = "limits-agent-slots";

pub(crate) fn limits_tab(desktop: &mut Desktop) {
  settings(desktop);
  desktop.click("settings-tab-limits");
  desktop.settle("the stored limits", |tree| {
    tree
      .get_element_by_id_mut(DAILY_BUDGET)
      .and_then(|element| element.as_text_input())
      .is_some_and(|input| !input.value().is_empty())
  });
}

/// The labels of the time limit select and the one chosen.
fn time_limit(desktop: &mut Desktop) -> (Vec<String>, Vec<String>) {
  let select = desktop
    .tree
    .get_element_by_id_mut(TIME_LIMIT)
    .and_then(|element| element.as_select())
    .expect("the time limit select");
  let strings = |labels: Vec<std::sync::Arc<str>>| labels.iter().map(|label| label.to_string()).collect();
  (strings(select.labels()), strings(select.selected_labels()))
}

pub(crate) fn stored_limits(repo: &Path) -> ProjectLimits {
  ProjectDb::open(repo)
    .expect("open the project")
    .project_limits()
    .expect("limits")
}

#[test]
fn project_settings_limits_show_the_defaults_and_their_fields_in_order() {
  let mut desktop = Desktop::with_project();
  limits_tab(&mut desktop);
  let description = "Defaults and limits for Kontur. They apply to new tasks and runs in this project.";
  assert!(desktop.tree.root().is_some_and(|root| contains_text(root, description)));
  // No run budget and no time limit by default, and no maximum run budget at all.
  assert_eq!(desktop.value(DEFAULT_BUDGET), "");
  assert_eq!(desktop.value(DAILY_BUDGET), "20,000,000");
  let (choices, chosen) = time_limit(&mut desktop);
  assert_eq!(chosen, ["No limit"]);
  assert_eq!(
    choices.iter().take(2).map(String::as_str).collect::<Vec<_>>(),
    ["No limit", "15 minutes"]
  );
  assert!(shows(&mut desktop, "settings-limits", "No limit"));
  assert!(shows(&mut desktop, "settings-limits", "= 20M tokens"));
  assert!(shows(&mut desktop, "settings-save-status", "All changes saved"));
  assert!(desktop.tree.get_element_by_id("settings-limits-notice").is_some());
  assert_eq!(desktop.value(AGENT_SLOTS), "4");
  assert_eq!(
    desktop.stops(5, false),
    expected(&[
      Some(DEFAULT_BUDGET),
      Some(DAILY_BUDGET),
      Some(TIME_LIMIT),
      Some(AGENT_SLOTS),
      Some(DEFAULT_BUDGET)
    ])
  );
}

#[test]
fn project_settings_limits_save_a_valid_change_when_its_field_is_left() {
  let mut desktop = Desktop::with_project();
  limits_tab(&mut desktop);
  desktop.tab_to(DEFAULT_BUDGET, 10);
  desktop.type_text("12x");
  assert!(shows(
    &mut desktop,
    "settings-limits",
    "Use digits only, for example 1,400,000."
  ));
  // Leaving an invalid field saves nothing.
  desktop.tab();
  assert_eq!(stored_limits(desktop.repo()), ProjectLimits::DEFAULT);
  desktop.shift_tab();
  desktop.key("Backspace", "Backspace");
  // No maximum: a default above the daily budget is a valid default.
  desktop.type_text("0000000");
  assert_eq!(desktop.value(DEFAULT_BUDGET), "120,000,000");
  desktop.tab();
  let expected_limits = ProjectLimits::new(
    Some(TokenBudget::new(120_000_000).expect("above zero")),
    ProjectLimits::DEFAULT.daily_budget(),
    ProjectLimits::DEFAULT.default_time_limit(),
  )
  .expect("valid limits");
  let repo = desktop.repo().to_path_buf();
  desktop.settle("the saved default", |_| stored_limits(&repo) == expected_limits);
  desktop.settle("the saved status", |tree| {
    tree
      .get_element_by_id("settings-save-status")
      .is_some_and(|status| contains_text(status, "All changes saved"))
  });
  // Emptied, the default is no limit again.
  desktop.shift_tab();
  for _ in 0.."120,000,000".len() {
    desktop.key("Backspace", "Backspace");
  }
  assert!(shows(&mut desktop, "settings-limits", "No limit"));
  desktop.tab();
  desktop.settle("no default budget", |_| stored_limits(&repo) == ProjectLimits::DEFAULT);
}

#[test]
fn project_settings_limits_save_a_time_limit_when_it_is_chosen() {
  let mut desktop = Desktop::with_project();
  limits_tab(&mut desktop);
  desktop.tab_to(DAILY_BUDGET, 10);
  desktop.tab();
  // The select opens on the chosen No limit; Down moves to 15 minutes and Enter chooses it.
  desktop.enter();
  desktop.key("ArrowDown", "ArrowDown");
  desktop.enter();
  assert_eq!(time_limit(&mut desktop).1, ["15 minutes"]);
  let repo = desktop.repo().to_path_buf();
  desktop.settle("the saved time limit", |_| {
    stored_limits(&repo).default_time_limit() == Some(std::time::Duration::from_secs(15 * 60))
  });
  assert!(shows(&mut desktop, "settings-limits", "active agent time"));
}

#[test]
fn project_settings_new_task_shows_the_linked_repository() {
  let mut desktop = linked("ivanzaida");
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-new-task");
  desktop.click("tasks-new-task");
  desktop.wait_for("new-task-base-branch");
  assert!(shows(&mut desktop, "new-task-repository", "ivanzaida/kontur"));
  assert!(shows(&mut desktop, "new-task-base-branch", "main"));
}

const VARIABLES: &str = "settings-environment-variables";

fn set_variables(desktop: &mut Desktop, text: &str) {
  desktop
    .tree
    .get_element_by_id_mut(VARIABLES)
    .and_then(|element| element.as_text_input())
    .expect("the variables field")
    .set_value(text);
  desktop.pass();
}

/// The variables field is the form's only stop, so Tab keeps the focus there: the field is left by
/// pressing elsewhere, which saves it.
fn leave_variables(desktop: &mut Desktop) {
  desktop.press("settings-environment-notice");
  assert_ne!(desktop.focused().as_deref(), Some(VARIABLES));
}

fn stored_environment(repo: &Path) -> Vec<(String, String)> {
  ProjectDb::open(repo)
    .expect("open")
    .project_environment()
    .expect("environment")
    .iter()
    .map(|(name, value)| (name.to_string(), value.to_owned()))
    .collect()
}

#[test]
fn project_settings_environment_saves_valid_variables() {
  let mut desktop = Desktop::with_project();
  let stored = CommandEnvironment::new(BTreeMap::from([(
    VariableName::new("LOG_LEVEL").expect("name"),
    "debug".to_owned(),
  )]))
  .expect("environment");
  ProjectDb::open(desktop.repo())
    .expect("open")
    .save_project_environment(&stored)
    .expect("saved");
  settings(&mut desktop);
  desktop.click("settings-tab-environment");
  desktop.wait_until("the stored variables", |desktop| {
    desktop.tree.get_element_by_id(VARIABLES).is_some() && desktop.value(VARIABLES) == "LOG_LEVEL=debug"
  });
  // A line that is not NAME=value says so and saves nothing when the field is left.
  desktop.tab_to(VARIABLES, 5);
  set_variables(&mut desktop, "LOG_LEVEL info");
  leave_variables(&mut desktop);
  assert!(shows(
    &mut desktop,
    "settings-environment",
    "Write each variable as NAME=value: LOG_LEVEL info"
  ));
  assert_eq!(
    stored_environment(desktop.repo()),
    [("LOG_LEVEL".to_owned(), "debug".to_owned())]
  );
  desktop.tab_to(VARIABLES, 5);
  set_variables(&mut desktop, "LOG_LEVEL=info");
  leave_variables(&mut desktop);
  let repo = desktop.repo().to_path_buf();
  desktop.settle("the saved variable", |_| {
    stored_environment(&repo) == [("LOG_LEVEL".to_owned(), "info".to_owned())]
  });
  // A variable that names a folder is stored as written, like any other.
  let folder = repo.join(".orchester").join("build").to_string_lossy().into_owned();
  desktop.tab_to(VARIABLES, 5);
  set_variables(&mut desktop, &format!("LOG_LEVEL=info\nBUILD_DIR={folder}"));
  leave_variables(&mut desktop);
  desktop.settle("the saved folder variable", |_| stored_environment(&repo).len() == 2);
  assert_eq!(
    stored_environment(&repo),
    [
      ("BUILD_DIR".to_owned(), folder),
      ("LOG_LEVEL".to_owned(), "info".to_owned())
    ]
  );
  desktop.settle("the saved status", |tree| {
    tree
      .get_element_by_id("settings-save-status")
      .is_some_and(|status| contains_text(status, "All changes saved"))
  });
}
