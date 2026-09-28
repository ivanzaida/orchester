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
fn long_project_name_keeps_overview_crumb_and_switcher_actions_visible() {
  let name = format!("U9 failure label {}", "LONGPROJECTNAME".repeat(10));
  let mut desktop = Desktop::with_project_named(&name);
  desktop.wait_for("overview-breadcrumb-current");
  assert!(shows(&mut desktop, "overview-breadcrumb-project", &name));

  for width in [1440, 900] {
    desktop.tree.resize(width, 960);
    desktop.pass();
    let project = desktop.bounds("overview-breadcrumb-project");
    let separator = desktop.bounds("overview-breadcrumb-separator");
    let current = desktop.bounds("overview-breadcrumb-current");
    let bar = desktop.bounds("overview-project-bar");
    assert!(
      project.x + project.width <= separator.x,
      "project crumb must give way at {width}px"
    );
    assert!(
      separator.x + separator.width <= current.x,
      "the current crumb must follow at {width}px"
    );
    assert!(
      current.x + current.width <= bar.x + bar.width,
      "Overview must stay visible at {width}px"
    );

    desktop.press("project-selector");
    desktop.wait_for("switcher-project-name-0");
    assert!(shows(&mut desktop, "switcher-project-name-0", &name));
    for hovered in [false, true] {
      if hovered {
        desktop.hover("switcher-project-0");
      }
      let label = desktop.bounds("switcher-project-name-0");
      let check = desktop.bounds("switcher-project-check-0");
      assert!(
        label.x + label.width <= check.x,
        "Recent label must leave its actions visible at {width}px"
      );
      if hovered {
        let new_tab = desktop.bounds("switcher-new-tab-0");
        assert!(
          label.x + label.width <= new_tab.x,
          "Recent label must leave the hovered new-tab action visible at {width}px"
        );
      }
    }
    desktop.press("project-selector");
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
