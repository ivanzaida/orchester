//! Settings › *Let automation change agent permissions* and *Let the Orchestrator publish drafts*: each off until
//! a person turns it on in the window, where it reads the stored value and saves what the person chooses.

mod support;

use lurq::app::events::MouseButton;
use orchester_app::routes::Route;
use support::Window;

const SWITCH: &str = "settings-change-permissions";
const PUBLISH: &str = "settings-publish-drafts";

/// Whether the switch shows on: its knob at the end of its track.
fn on(window: &mut Window) -> bool {
  switch_on(window, SWITCH)
}

/// Whether the switch `switch` shows on.
fn switch_on(window: &mut Window, switch: &str) -> bool {
  let mut bounds = |id: String| {
    window
      .tree
      .get_element_by_id_mut(&id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let (track, knob) = (bounds(format!("{switch}-mark")), bounds(format!("{switch}-knob")));
  knob.x - track.x > track.width / 2.0 - knob.width / 2.0
}

/// Presses the switch until the stored setting is `wanted`; a press while it is read or saved is not
/// taken, and the setting is read before each press, so it never flips back.
fn turn(window: &mut Window, wanted: bool) {
  let workspace = window.launch.workspace().expect("the session started").clone();
  window.until("the choice to be saved", |window| {
    let stored = workspace.automation_changes_permissions().expect("the setting reads");
    if stored != wanted {
      click(window, SWITCH);
    }
    stored == wanted
  });
  window.until("the switch to show it", |window| on(window) == wanted);
}

fn click(window: &mut Window, id: &str) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, MouseButton::Left);
  window.tree.mouse_up(x, y, MouseButton::Left);
  window.pass();
}

#[test]
fn settings_a_person_turns_on_automation_changing_agent_permissions() {
  let mut window = Window::open();
  window.navigation().go(Route::Settings);
  window.until("the setting's row", |window| {
    window.texts("settings-automation").is_some()
  });
  let texts = window.texts("settings-automation").expect("the section shows");
  assert!(
    texts
      .iter()
      .any(|text| text == "Let automation change agent permissions"),
    "{texts:?}"
  );
  assert!(!on(&mut window), "off until turned on");
  turn(&mut window, true);
  turn(&mut window, false);
}

#[test]
fn settings_human_revocation_survives_leaving_before_save_dispatch() {
  let mut window = Window::open();
  let workspace = window.launch.workspace().expect("session").clone();
  workspace
    .set_automation_changes_permissions(true)
    .expect("private human opt-in");
  window.navigation().go(Route::Settings);
  window.until("enabled switch", on);
  let bounds = window
    .tree
    .get_element_by_id_mut(SWITCH)
    .and_then(|element| element.bounds())
    .expect("switch bounds");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, MouseButton::Left);
  window.tree.mouse_up(x, y, MouseButton::Left);
  // Do not tick the future: leave immediately after the actual human pointer event.
  assert!(
    !workspace.automation_permission_consent(),
    "human event revokes synchronously"
  );
  window.navigation().go(Route::AllProjects);
  window.pass();
  assert!(!window.link.input_gate().protecting(), "Settings has unmounted");
  assert!(
    !workspace
      .automation_changes_permissions()
      .expect("stored read after leaving")
  );
  assert!(
    !workspace.automation_permission_consent(),
    "leaving cannot restore the old grant"
  );
}

#[test]
fn settings_synthetic_pointer_cannot_change_its_own_opt_in() {
  use lurq::app::events::with_synthetic_input;
  let mut window = Window::open();
  let workspace = window.launch.workspace().expect("session").clone();
  window.navigation().go(Route::Settings);
  window.until("loaded Settings switch", |window| {
    window.texts("settings-automation").is_some()
  });
  for chosen in [false, true] {
    workspace
      .set_automation_changes_permissions(chosen)
      .expect("private human choice");
    // Remount to show the stored human choice, with no MCP caller in flight.
    window.navigation().go(Route::AllProjects);
    window.pass();
    window.navigation().go(Route::Settings);
    window.until("stored switch", |window| on(window) == chosen);
    window.until("synthetic input rejected", |window| {
      with_synthetic_input(|| click(window, SWITCH));
      window.texts("settings-change-permissions-blocked").is_some()
    });
    assert_eq!(
      workspace.automation_changes_permissions().expect("actual setting"),
      chosen
    );
    assert!(
      window.texts("settings-change-permissions-blocked").is_some(),
      "synthetic input is visibly rejected"
    );
  }
}

/// Clicks the element `id` where it is, laid out below the window's fold or not.
fn press_element(window: &mut Window, id: &str) {
  window
    .tree
    .get_element_by_id_mut(id)
    .unwrap_or_else(|| panic!("{id} exists"))
    .click();
  window.pass();
}

#[test]
fn settings_only_a_person_lets_the_orchestrator_publish_drafts() {
  use lurq::app::events::with_synthetic_input;
  let mut window = Window::open();
  let workspace = window.launch.workspace().expect("session").clone();
  window.navigation().go(Route::Settings);
  window.until("the setting's row", |window| {
    window.texts(&format!("{PUBLISH}-row")).is_some()
  });
  let texts = window.texts(&format!("{PUBLISH}-row")).expect("the row shows");
  assert!(
    texts.iter().any(|text| text == "Let the Orchestrator publish drafts"),
    "{texts:?}"
  );
  assert!(!switch_on(&mut window, PUBLISH), "off until turned on");
  window.until("synthetic input rejected", |window| {
    with_synthetic_input(|| press_element(window, PUBLISH));
    window.texts(&format!("{PUBLISH}-blocked")).is_some()
  });
  assert!(!workspace.orchestrator_publishes_drafts().expect("the setting reads"));
  window.until("the person's choice to be saved", |window| {
    let stored = workspace.orchestrator_publishes_drafts().expect("the setting reads");
    if !stored {
      press_element(window, PUBLISH);
    }
    stored
  });
  window.until("the switch to show it", |window| switch_on(window, PUBLISH));
  assert!(
    !workspace
      .automation_changes_permissions()
      .expect("the other setting reads"),
    "publishing drafts turns nothing else on"
  );
}

#[test]
fn settings_turning_off_publishing_drafts_holds_from_the_click() {
  let mut window = Window::open();
  let workspace = window.launch.workspace().expect("session").clone();
  workspace
    .set_orchestrator_publishes_drafts(true)
    .expect("the person turned it on");
  window.navigation().go(Route::Settings);
  window.until("the switch on", |window| {
    window.texts(&format!("{PUBLISH}-row")).is_some() && switch_on(window, PUBLISH)
  });
  // The person's click; its save has not run yet.
  window.tree.get_element_by_id_mut(PUBLISH).expect("the switch").click();
  assert!(
    !workspace.orchestrator_publishes_drafts().expect("read"),
    "the chat stops publishing from the click, before the save"
  );
}
