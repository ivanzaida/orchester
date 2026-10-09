//! Settings › *Let automation change agent permissions*: off until a person turns it on in the window,
//! where it reads the stored value and saves what the person chooses.

mod support;

use lurq::app::events::MouseButton;
use orchester_app::routes::Route;
use support::Window;

const SWITCH: &str = "settings-change-permissions";

/// Whether the switch shows on: its knob at the end of its track.
fn on(window: &mut Window) -> bool {
  let mut bounds = |id: String| {
    window
      .tree
      .get_element_by_id_mut(&id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let (track, knob) = (bounds(format!("{SWITCH}-mark")), bounds(format!("{SWITCH}-knob")));
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
