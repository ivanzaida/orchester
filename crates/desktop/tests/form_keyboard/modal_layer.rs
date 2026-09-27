//! A dialog's modal layer covers the screen and nothing else, as the design draws every backdrop
//! over its screen frame (`a8U756`, `nqqYL` in `c3jD7`): the scrim leaves the title bar and the
//! status bar out, and the dialog is centred in the screen between them. While a dialog is open,
//! only the dialog and the title bar take input: the status bar under it shows no hint and its
//! items open nothing, and Tab stays in the dialog.

use lurq::core::ElementRect;
use orchester_desktop::theme::{STATUS_BAR_HEIGHT, TITLE_BAR_HEIGHT};

use crate::harness::Desktop;

/// The harness window width.
const WINDOW_WIDTH: f32 = 1440.0;
/// Layout rounding.
const EPSILON: f32 = 0.5;
const PIPELINE_NAME: &str = "new-pipeline-name";

fn new_pipeline_dialog() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-pipelines");
  desktop.click("nav-pipelines");
  desktop.wait_for("pipelines-new");
  desktop.click("pipelines-new");
  desktop.wait_for(PIPELINE_NAME);
  desktop
}

fn connection_dialog() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-connections");
  desktop.click("nav-connections");
  desktop.wait_for("add-connection");
  desktop.click("add-connection");
  desktop.wait_for("connection-dialog");
  desktop
}

fn bottom(rect: &ElementRect) -> f32 {
  rect.y + rect.height
}

fn right(rect: &ElementRect) -> f32 {
  rect.x + rect.width
}

fn assert_near(actual: f32, expected: f32, what: &str) {
  assert!(
    (actual - expected).abs() <= EPSILON,
    "{what}: {actual} instead of {expected}"
  );
}

/// The scrim spans the window between the title bar and the status bar, and `dialog` is centred
/// in it.
fn assert_dialog_in_screen(desktop: &mut Desktop, dialog: &str) {
  let scrim = desktop.bounds("dialog-backdrop");
  let bar = desktop.bounds("status-bar");
  let dialog = desktop.bounds(dialog);
  assert_near(bar.height, STATUS_BAR_HEIGHT, "status bar height");
  assert_near(scrim.y, TITLE_BAR_HEIGHT, "scrim top");
  assert_near(bottom(&scrim), bar.y, "scrim bottom");
  assert_near(scrim.x, 0.0, "scrim left");
  assert_near(scrim.width, WINDOW_WIDTH, "scrim width");
  assert!(bottom(&dialog) <= bar.y, "the dialog reaches into the status bar");
  assert_near(
    dialog.y - scrim.y,
    bottom(&scrim) - bottom(&dialog),
    "space above the dialog",
  );
  assert_near(
    dialog.x - scrim.x,
    right(&scrim) - right(&dialog),
    "space left of the dialog",
  );
}

#[test]
fn modal_layer_new_pipeline_dialog_is_centred_in_the_screen_above_the_status_bar() {
  let mut desktop = new_pipeline_dialog();
  assert_dialog_in_screen(&mut desktop, "new-pipeline-dialog");
}

#[test]
fn modal_layer_connection_dialog_is_centred_in_the_screen_above_the_status_bar() {
  let mut desktop = connection_dialog();
  assert_dialog_in_screen(&mut desktop, "connection-dialog");
}

#[test]
fn modal_layer_status_bar_under_a_dialog_shows_no_hint() {
  let mut desktop = new_pipeline_dialog();
  desktop.hover("status-version");
  assert!(
    desktop.tree.get_element_by_id("status-bar-hint").is_none(),
    "the version under a dialog shows its hint"
  );
}

#[test]
fn modal_layer_status_bar_under_a_dialog_opens_nothing() {
  let mut desktop = new_pipeline_dialog();
  desktop.type_text("Release");
  for item in ["status-queued", "status-agents", "status-needs-you", "status-runner"] {
    desktop.press(item);
    assert!(
      desktop.tree.get_element_by_id("new-pipeline-dialog").is_some(),
      "{item} under the dialog closed it"
    );
    assert_eq!(
      desktop.value(PIPELINE_NAME),
      "Release",
      "{item} under the dialog lost the name"
    );
    assert_eq!(
      desktop.focused().as_deref(),
      Some(PIPELINE_NAME),
      "{item} under the dialog took the focus"
    );
  }
  // Tab goes on inside the dialog: from the name to the first starting point.
  desktop.tab();
  assert_eq!(desktop.focused().as_deref(), Some("new-pipeline-starter"));
}
