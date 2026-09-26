//! Project settings (02.N General, 02.M Limits) and the linked repository in New task. Only the
//! settings form's buttons take Tab stops; the tabs are pointer-only. Buttons that run `gh` or
//! `git` are not pressed: the tests never run the real programs.

use std::path::Path;

use orchester_core::{limits::ProjectLimits, store::SettingsStore, tokens::TokenBudget};
use orchester_store::ProjectDb;

use crate::{
  github::{ReplayGh, SIGNED_IN, VERSION, contains_text, github_connection, link, shows},
  harness::{Desktop, expected},
};

const REPOSITORY: &str = "settings-repository";
const CHECK: &str = "settings-check-repository";
const USE_SIGNED_IN: &str = "settings-use-signed-in";
const DETECT: &str = "settings-detect-repository";

fn settings(desktop: &mut Desktop) {
  desktop.wait_for("nav-settings");
  desktop.click("nav-settings");
  desktop.wait_for(REPOSITORY);
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
  // The settings form: Check again and Use signed-in account; the tabs and the sidebar are
  // pointer-only.
  assert_eq!(
    desktop.stops(3, false),
    expected(&[Some(CHECK), Some(USE_SIGNED_IN), Some(CHECK)])
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
  assert_eq!(desktop.stops(2, false), expected(&[Some(CHECK), Some(CHECK)]));
}

#[test]
fn project_settings_without_a_link_offer_detection() {
  let mut desktop = Desktop::with_project();
  settings(&mut desktop);
  assert!(shows(&mut desktop, REPOSITORY, "Not linked"));
  assert!(desktop.tree.get_element_by_id("settings-expected-account").is_none());
  assert!(desktop.tree.get_element_by_id("github-detection").is_none());
  assert_eq!(desktop.stops(2, false), expected(&[Some(DETECT), Some(DETECT)]));
}

const MAX_BUDGET: &str = "limits-max-run-budget";
const DEFAULT_BUDGET: &str = "limits-default-run-budget";
const DAILY_BUDGET: &str = "limits-daily-budget";
const TIME_LIMIT: &str = "limits-default-time-limit";

fn limits_tab(desktop: &mut Desktop) {
  settings(desktop);
  desktop.click("settings-tab-limits");
  desktop.settle("the stored limits", |tree| {
    tree
      .get_element_by_id_mut(MAX_BUDGET)
      .and_then(|element| element.as_text_input())
      .is_some_and(|input| !input.value().is_empty())
  });
}

fn stored_limits(repo: &Path) -> ProjectLimits {
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
  assert_eq!(desktop.value(MAX_BUDGET), "10,000,000");
  assert_eq!(desktop.value(DEFAULT_BUDGET), "1,400,000");
  assert_eq!(desktop.value(DAILY_BUDGET), "20,000,000");
  assert_eq!(desktop.value(TIME_LIMIT), "30");
  assert!(shows(&mut desktop, "settings-limits", "= 10M tokens · default"));
  assert!(shows(&mut desktop, "settings-save-status", "All changes saved"));
  assert!(desktop.tree.get_element_by_id("settings-limits-notice").is_some());
  assert_eq!(
    desktop.stops(5, false),
    expected(&[
      Some(MAX_BUDGET),
      Some(DEFAULT_BUDGET),
      Some(DAILY_BUDGET),
      Some(TIME_LIMIT),
      Some(MAX_BUDGET)
    ])
  );
}

#[test]
fn project_settings_limits_save_a_valid_change_when_its_field_is_left() {
  let mut desktop = Desktop::with_project();
  limits_tab(&mut desktop);
  desktop.tab_to(DEFAULT_BUDGET, 10);
  for _ in 0.."1,400,000".len() {
    desktop.key("Backspace", "Backspace");
  }
  desktop.type_text("12000000");
  assert!(shows(
    &mut desktop,
    "settings-limits",
    "The default can be at most the maximum run budget, 10M tokens."
  ));
  // Leaving an invalid field saves nothing.
  desktop.tab();
  assert_eq!(stored_limits(desktop.repo()), ProjectLimits::DEFAULT);
  desktop.shift_tab();
  desktop.key("Backspace", "Backspace");
  assert_eq!(desktop.value(DEFAULT_BUDGET), "1,200,000");
  desktop.tab();
  let expected_limits = ProjectLimits::new(
    ProjectLimits::DEFAULT.max_run_budget(),
    TokenBudget::new(1_200_000).expect("above zero"),
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
