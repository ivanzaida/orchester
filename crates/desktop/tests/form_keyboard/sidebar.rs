//! The shell sidebar (`zo9Py`): the current project with its switch mark, and the runner card
//! (`O2Jjjf`, states `O5NoB1`) above Project settings, which reads where the project's runner
//! stands. The card with the runner online is covered in `lifecycle`.

use crate::{github::shows, harness::Desktop};

#[test]
fn sidebar_shows_the_project_switch_mark_in_the_selector() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("project-selector");
  let selector = desktop.bounds("project-selector");
  let switch = desktop.bounds("project-selector-switch");
  assert!(
    switch.x > selector.x && switch.x + switch.width < selector.x + selector.width,
    "the switch mark sits inside the selector"
  );
}

#[test]
fn long_project_name_stays_inside_selector_and_overview_heading() {
  let name = format!("U7 failure label {}", "LONGPROJECTNAME".repeat(10));
  let mut desktop = Desktop::with_project_named(&name);
  desktop.wait_for("overview-project-name");
  assert!(shows(&mut desktop, "project-selector-name", &name));
  assert!(shows(&mut desktop, "overview-project-name", &name));

  for width in [1440, 900] {
    desktop.tree.resize(width, 960);
    desktop.pass();
    let selector = desktop.bounds("project-selector");
    let selector_name = desktop.bounds("project-selector-name");
    let switch = desktop.bounds("project-selector-switch");
    let heading_name = desktop.bounds("overview-project-name");
    let new_task = desktop.bounds("new-task");
    assert!(
      selector_name.x >= selector.x && selector_name.x + selector_name.width <= switch.x,
      "selector name must fit before the switch at {width}px"
    );
    assert!(
      heading_name.x + heading_name.width < new_task.x,
      "overview heading must leave the New task button visible at {width}px"
    );
  }
}

#[test]
fn sidebar_runner_card_says_the_runner_did_not_start() {
  // The harness has no runner program, so the runner cannot start.
  let mut desktop = Desktop::with_project();
  desktop.wait_until("the runner given up", |desktop| {
    desktop.tree.get_element_by_id("sidebar-runner-state").is_some()
      && shows(desktop, "sidebar-runner-state", "Runner offline")
  });
  assert!(shows(
    &mut desktop,
    "sidebar-runner-detail",
    "Could not start · see runner log"
  ));
  let card = desktop.bounds("sidebar-runner");
  let settings = desktop.bounds("nav-settings");
  assert!(
    card.y + card.height < settings.y,
    "the card sits above Project settings"
  );
}
