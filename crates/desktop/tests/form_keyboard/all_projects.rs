//! All projects (01.06) and the sidebar's project switcher (01.06 B): the switcher opens another
//! project as a new history entry, All projects lists the workspace's projects and opens them,
//! removing one asks first and never touches its files, a project whose folder is gone is listed
//! as missing, and a start whose last project is gone lands on All projects.

use std::fs;

use lurq::app::events::MouseButton;

use crate::{
  github::{contains_text, shows},
  harness::{Desktop, expected},
  task_run::{create_and_run, init_repository, programs, seed},
};

const BACK: MouseButton = MouseButton::Other(4);

/// Kontur, shown, and Personal website, opened before it.
fn two_projects() -> Desktop {
  let mut desktop = Desktop::with_other_project(|_| {}, |_| {});
  desktop.wait_for("project-selector");
  desktop
}

fn has(desktop: &mut Desktop, id: &str) -> bool {
  desktop.tree.get_element_by_id(id).is_some()
}

fn back(desktop: &mut Desktop) {
  desktop.tree.mouse_down(720.0, 480.0, BACK);
  desktop.pass();
  desktop.tree.mouse_up(720.0, 480.0, BACK);
  desktop.pass();
}

/// All projects opened from the sidebar, with both rows read.
fn all_projects() -> Desktop {
  let mut desktop = two_projects();
  desktop.press("nav-all-projects");
  desktop.wait_for("project-row-1");
  desktop
}

#[test]
fn all_projects_switcher_opens_another_project_that_back_leaves() {
  let mut desktop = two_projects();
  desktop.press("project-selector");
  desktop.wait_for("switcher-project-1");
  assert!(
    shows(&mut desktop, "switcher-project-0", "Kontur"),
    "the shown project comes first"
  );
  assert!(shows(&mut desktop, "project-switcher", "All projects"));
  assert!(shows(&mut desktop, "project-switcher", "Add project"));
  desktop.press("switcher-project-1");
  desktop.settle("Personal website shown", |tree| {
    tree.get_element_by_id("project-switcher").is_none()
      && tree
        .get_element_by_id("project-selector")
        .is_some_and(|selector| contains_text(selector, "Personal website"))
  });
  back(&mut desktop);
  desktop.settle("Kontur shown again", |tree| {
    tree
      .get_element_by_id("project-selector")
      .is_some_and(|selector| contains_text(selector, "Kontur"))
  });
}

#[test]
fn all_projects_switcher_leads_to_all_projects_and_setup() {
  let mut desktop = two_projects();
  desktop.press("project-selector");
  desktop.wait_for("switcher-all-projects");
  desktop.press("switcher-all-projects");
  desktop.wait_for("all-projects");
  back(&mut desktop);
  desktop.wait_for("project-selector");
  desktop.press("project-selector");
  desktop.wait_for("switcher-add-project");
  desktop.press("switcher-add-project");
  desktop.wait_for("setup-back");
  desktop.press("setup-back");
  desktop.wait_for("project-selector");
}

#[test]
fn all_projects_lists_the_projects_with_their_runners_and_opens_one() {
  let mut desktop = all_projects();
  assert!(shows(&mut desktop, "project-row-0", "Kontur"));
  assert!(shows(&mut desktop, "project-row-1", "Personal website"));
  // The harness has no runner program: Kontur's runner could not start; Personal website's was
  // never started.
  desktop.settle("Kontur's runner offline", |tree| {
    tree
      .get_element_by_id("project-row-status-0")
      .is_some_and(|status| contains_text(status, "Runner offline"))
  });
  assert!(shows(&mut desktop, "project-row-status-1", "Runner not started"));
  // Page actions and rows are pointer-only.
  desktop.tab();
  assert_eq!(desktop.focused(), None);
  desktop.press("project-row-1");
  desktop.settle("Personal website shown", |tree| {
    tree
      .get_element_by_id("project-selector")
      .is_some_and(|selector| contains_text(selector, "Personal website"))
  });
}

#[test]
fn all_projects_removal_asks_first_and_keeps_the_files() {
  let mut desktop = all_projects();
  desktop.press("project-remove-1");
  desktop.wait_for("remove-project-dialog");
  assert!(shows(
    &mut desktop,
    "remove-project-dialog",
    "Remove Personal website from the list?"
  ));
  assert!(shows(&mut desktop, "remove-project-kept", "stays on disk"));
  assert_eq!(
    desktop.stops(3, false),
    expected(&[
      Some("remove-project-cancel"),
      Some("remove-project-confirm"),
      Some("remove-project-cancel")
    ])
  );
  desktop.key("Escape", "Escape");
  assert!(!has(&mut desktop, "remove-project-dialog"), "Escape closes it");
  assert!(has(&mut desktop, "project-row-1"), "nothing was removed");

  desktop.press("project-remove-1");
  desktop.wait_for("remove-project-dialog");
  desktop.tab_to("remove-project-confirm", 3);
  desktop.enter();
  desktop.settle("the project removed from the list", |tree| {
    tree.get_element_by_id("remove-project-dialog").is_none() && tree.get_element_by_id("project-row-1").is_none()
  });
  assert!(shows(&mut desktop, "project-row-0", "Kontur"));
  let data = desktop.other_repo().join(".orchester").join("orchester.db");
  assert!(data.is_file(), "the removed project's data stays on disk");
}

#[test]
fn all_projects_removing_the_shown_project_leaves_its_history() {
  let mut desktop = all_projects();
  desktop.press("project-remove-0");
  desktop.wait_for("remove-project-confirm");
  desktop.press("remove-project-confirm");
  desktop.settle("Kontur removed", |tree| {
    tree.get_element_by_id("project-row-1").is_none()
  });
  back(&mut desktop);
  desktop.settle("All projects instead of the removed project", |tree| {
    tree.get_element_by_id("all-projects").is_some()
  });
  assert!(!has(&mut desktop, "project-selector"));
}

#[test]
fn all_projects_lists_a_project_whose_folder_is_gone_as_missing() {
  let mut desktop = two_projects();
  fs::remove_dir_all(desktop.other_repo()).expect("remove the other project's folder");
  desktop.press("nav-all-projects");
  desktop.wait_for("project-row-status-1");
  assert!(shows(&mut desktop, "project-row-status-1", "Folder missing"));
  assert!(shows(
    &mut desktop,
    "project-row-why-1",
    "The folder is not there any more"
  ));
  assert!(shows(&mut desktop, "project-row-1", "—"), "its counts are not known");
  desktop.press("project-row-1");
  assert!(has(&mut desktop, "all-projects"), "a missing project does not open");
  assert!(has(&mut desktop, "project-remove-1"), "it can still be removed");
}

#[test]
fn all_projects_is_where_a_start_lands_when_the_last_project_is_gone() {
  let mut desktop = Desktop::with_seeded_project(|repo| fs::remove_dir_all(repo).expect("remove the folder"));
  desktop.wait_for("project-row-status-0");
  assert!(shows(&mut desktop, "project-row-status-0", "Folder missing"));
  assert!(!has(&mut desktop, "project-selector"));
}

#[test]
fn all_projects_counts_come_from_each_project() {
  let mut desktop = all_projects();
  // Neither project has runs: once read, their counts are 0 rather than unknown.
  desktop.settle("the counts read", |tree| {
    tree
      .get_element_by_id("project-row-1")
      .is_some_and(|row| !contains_text(row, "—"))
  });
}

#[test]
fn all_projects_removal_waits_while_agents_are_at_work() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_until("Pause run", |desktop| has(desktop, "task-pause-run"));
  desktop.press("nav-all-projects");
  desktop.wait_for("project-remove-0");
  desktop.press("project-remove-0");
  desktop.wait_until("the reason", |desktop| has(desktop, "remove-project-problem"));
  assert!(shows(
    &mut desktop,
    "remove-project-problem",
    "Agents are at work in Kontur. Remove it once its runs are paused or finished."
  ));
  // Remove from list cannot be pressed; the reason wraps left of the buttons (01.06 F `glurU`).
  assert_eq!(
    desktop.stops(2, false),
    expected(&[Some("remove-project-cancel"), Some("remove-project-cancel")])
  );
  let (problem, cancel) = (
    desktop.bounds("remove-project-problem"),
    desktop.bounds("remove-project-cancel"),
  );
  assert!(
    problem.x + problem.width <= cancel.x,
    "{problem:?} runs under {cancel:?}"
  );
  assert!(
    problem.height > cancel.height / 2.0,
    "the reason wraps onto a second line"
  );
}
