//! Zooming the pipeline canvas: the zoom controls, the zoom shortcuts (which stay with a focused
//! text field), the wheel with and without the zoom modifier, fitting the pipeline into view,
//! and each pipeline keeping its zoom for the session.

use lurq::{
  app::{Tree, events::ScrollPhase},
  node::element::ElementRef,
};

use crate::harness::Desktop;

const LEVEL: &str = "pipeline-zoom-full";
const NAME: &str = "step-name";
/// More than the editor has stops, to find one by tabbing.
const MAX_STOPS: usize = 80;

/// The starter pipeline open in the editor.
fn starter() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-pipelines");
  desktop.press("nav-pipelines");
  desktop.wait_for("pipelines-create-starter");
  desktop.click("pipelines-create-starter");
  desktop.wait_for(NAME);
  desktop
}

fn text(element: ElementRef<'_>) -> Option<String> {
  element
    .text_content()
    .map(str::to_owned)
    .or_else(|| element.children().into_iter().find_map(text))
}

/// The zoom level the controls show, e.g. `100%`.
fn level(desktop: &mut Desktop) -> String {
  desktop
    .tree
    .get_element_by_id(LEVEL)
    .and_then(text)
    .expect("the zoom controls show the level")
}

/// The zoom modifier, Ctrl or Cmd on macOS, pressed or released.
fn modifier(tree: &mut Tree, down: bool) {
  let (key, code) = if cfg!(target_os = "macos") {
    ("Super", "SuperLeft")
  } else {
    ("Control", "ControlLeft")
  };
  let (ctrl, meta) = (down && !cfg!(target_os = "macos"), down && cfg!(target_os = "macos"));
  if down {
    tree.key_down_with_meta(key.into(), code.into(), false, ctrl, false, meta);
  } else {
    tree.key_up_with_meta(key.into(), code.into(), false, ctrl, false, meta);
  }
}

/// A shortcut with the zoom modifier.
fn shortcut(desktop: &mut Desktop, key: &str, code: &str) {
  let macos = cfg!(target_os = "macos");
  desktop
    .tree
    .key_down_with_meta(key.into(), code.into(), false, !macos, false, macos);
  desktop.pass();
}

/// A wheel notch away from the user over the middle of the canvas.
fn wheel(desktop: &mut Desktop, delta_y: f32) {
  let bounds = desktop
    .tree
    .get_element_by_id_mut("pipeline-canvas-area")
    .and_then(|element| element.bounds())
    .expect("the canvas is laid out");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
  desktop.tree.scroll(x, y, 0.0, delta_y, ScrollPhase::Scroll);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
  desktop.pass();
}

#[test]
fn canvas_zoom_controls_step_and_stop_at_the_ends() {
  let mut desktop = starter();
  assert_eq!(level(&mut desktop), "100%");
  desktop.press("pipeline-zoom-in");
  assert_eq!(level(&mut desktop), "110%");
  desktop.press("pipeline-zoom-out");
  desktop.press("pipeline-zoom-out");
  assert_eq!(level(&mut desktop), "90%");
  for _ in 0..12 {
    desktop.press("pipeline-zoom-out");
  }
  assert_eq!(level(&mut desktop), "25%");
  desktop.press(LEVEL);
  assert_eq!(level(&mut desktop), "100%");
  for _ in 0..12 {
    desktop.press("pipeline-zoom-in");
  }
  assert_eq!(level(&mut desktop), "200%");
}

#[test]
fn canvas_zoom_shortcuts_work_unless_a_text_field_has_the_focus() {
  let mut desktop = starter();
  shortcut(&mut desktop, "=", "Equal");
  assert_eq!(level(&mut desktop), "110%");
  shortcut(&mut desktop, "+", "Equal");
  assert_eq!(level(&mut desktop), "125%");
  shortcut(&mut desktop, "-", "Minus");
  assert_eq!(level(&mut desktop), "110%");
  shortcut(&mut desktop, "0", "Digit0");
  assert_eq!(level(&mut desktop), "100%");
  // Without the modifier the keys do nothing to the canvas.
  desktop.key("-", "Minus");
  assert_eq!(level(&mut desktop), "100%");
  // A focused text field keeps them.
  desktop.tab_to(NAME, MAX_STOPS);
  shortcut(&mut desktop, "=", "Equal");
  assert_eq!(level(&mut desktop), "100%");
}

#[test]
fn canvas_wheel_zooms_with_the_modifier_and_pans_without_it() {
  let mut desktop = starter();
  modifier(&mut desktop.tree, true);
  wheel(&mut desktop, 40.0);
  assert_eq!(level(&mut desktop), "110%");
  wheel(&mut desktop, -40.0);
  assert_eq!(level(&mut desktop), "100%");
  modifier(&mut desktop.tree, false);
  wheel(&mut desktop, 40.0);
  assert_eq!(level(&mut desktop), "100%");
}

#[test]
fn canvas_fit_shows_the_whole_pipeline_smaller_in_a_smaller_window() {
  let mut desktop = starter();
  let percent = |desktop: &mut Desktop| -> u32 {
    let shown = level(desktop);
    shown.trim_end_matches('%').parse().expect("a percentage")
  };
  desktop.press("pipeline-zoom-fit");
  let fitted = percent(&mut desktop);
  // The starter is a little taller than the canvas under its problems strip; fit never enlarges.
  assert!((50..100).contains(&fitted), "{fitted}%");
  desktop.tree.resize(1100, 700);
  desktop.pass();
  desktop.press("pipeline-zoom-fit");
  let smaller = percent(&mut desktop);
  assert!((25..fitted).contains(&smaller), "{smaller}% after {fitted}%");
  desktop.press(LEVEL);
  assert_eq!(level(&mut desktop), "100%");
}

#[test]
fn canvas_zoom_is_kept_for_the_pipeline_when_it_is_opened_again() {
  let mut desktop = starter();
  desktop.press("pipeline-zoom-out");
  assert_eq!(level(&mut desktop), "90%");
  desktop.press("pipeline-breadcrumb-library");
  desktop.wait_for("pipeline-row");
  desktop.press("pipeline-row");
  desktop.wait_for(LEVEL);
  assert_eq!(level(&mut desktop), "90%");
}
