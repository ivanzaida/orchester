//! Project settings (02.N General, 02.M Limits) and the linked repository in New task. Only the
//! settings form's buttons take Tab stops; the tabs are pointer-only. Buttons that run `gh` or
//! `git` are not pressed: the tests never run the real programs.

use crate::{
  github::{ReplayGh, SIGNED_IN, VERSION, github_connection, link, shows},
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

#[test]
fn project_settings_limits_are_unavailable() {
  let mut desktop = Desktop::with_project();
  settings(&mut desktop);
  desktop.click("settings-tab-limits");
  desktop.wait_for("settings-limits-unavailable");
  assert!(shows(
    &mut desktop,
    "settings-limits-unavailable",
    "Limits arrive with task runs"
  ));
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
