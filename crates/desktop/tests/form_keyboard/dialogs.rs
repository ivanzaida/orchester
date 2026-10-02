//! Dialogs are modals: Tab and Shift+Tab stay inside the open one, Enter submits its form only
//! when it can be submitted, and Escape does what each dialog allows.

use crate::harness::Desktop;

/// More presses than a dialog has stops, to see Tab wrap inside it.
const PRESSES: usize = 20;

const PIPELINE_DIALOG: &str = "new-pipeline-dialog";
const PIPELINE_NAME: &str = "new-pipeline-name";
const CONNECTION_DIALOG: &str = "connection-dialog";

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
  desktop.wait_for(CONNECTION_DIALOG);
  desktop
}

fn shown(desktop: &mut Desktop, id: &str) -> bool {
  desktop.tree.get_element_by_id(id).is_some()
}

/// Presses Tab `PRESSES` times and fails if the focus ever leaves the dialog's controls.
fn assert_tab_stays_in(desktop: &mut Desktop, controls: &[&str]) {
  for stop in desktop.stops(PRESSES, false) {
    let stop = stop.unwrap_or_default();
    assert!(
      stop.is_empty() || controls.contains(&stop.as_str()),
      "Tab left the dialog for {stop}"
    );
  }
}

#[test]
fn new_pipeline_dialog_enter_in_the_name_creates_the_pipeline() {
  let mut desktop = new_pipeline_dialog();
  assert_eq!(desktop.focused().as_deref(), Some(PIPELINE_NAME));
  desktop.type_text("Release");
  desktop.enter();
  desktop.settle("the new pipeline in the editor", |tree| {
    tree.get_element_by_id("pipeline-breadcrumb-library").is_some()
  });
}

#[test]
fn new_pipeline_dialog_enter_without_a_name_creates_nothing() {
  let mut desktop = new_pipeline_dialog();
  desktop.enter();
  assert!(shown(&mut desktop, PIPELINE_DIALOG));
}

#[test]
fn new_pipeline_dialog_escape_closes_it() {
  let mut desktop = new_pipeline_dialog();
  desktop.key("Escape", "Escape");
  assert!(!shown(&mut desktop, PIPELINE_DIALOG));
}

#[test]
fn new_pipeline_dialog_traps_tab() {
  let mut desktop = new_pipeline_dialog();
  // Create stays disabled without a name and is skipped.
  assert_tab_stays_in(
    &mut desktop,
    &[
      PIPELINE_NAME,
      "new-pipeline-starter",
      "new-pipeline-empty",
      "new-pipeline-cancel",
    ],
  );
  desktop.tab_to("new-pipeline-cancel", PRESSES);
  assert_eq!(desktop.stops(1, false).pop().flatten().as_deref(), Some(PIPELINE_NAME));
}

#[test]
fn connection_dialog_escape_keeps_it_open() {
  let mut desktop = connection_dialog();
  assert_eq!(desktop.focused().as_deref(), Some(CONNECTION_DIALOG));
  desktop.key("Escape", "Escape");
  assert!(shown(&mut desktop, CONNECTION_DIALOG));
}

#[test]
fn connection_dialog_enter_saves_nothing_without_a_program() {
  let mut desktop = connection_dialog();
  desktop
    .tree
    .get_element_by_id_mut("connection-name")
    .expect("connection name")
    .focus();
  desktop.enter();
  assert!(shown(&mut desktop, CONNECTION_DIALOG));
}

#[test]
fn connection_dialog_traps_tab() {
  let mut desktop = connection_dialog();
  // Selects have no id of their own and show up as unnamed stops. The OpenAI kind cannot be chosen
  // in this version, so it is no stop.
  assert_tab_stays_in(
    &mut desktop,
    &[
      "connection-dialog-close",
      "connection-name",
      "connection-kind-codex",
      "connection-kind-claude",
      "connection-kind-anthropic",
      "connection-program",
      "connection-recheck",
      "connection-cancel",
    ],
  );
  desktop.tab_to("connection-cancel", PRESSES);
  desktop.tab_to("connection-dialog-close", PRESSES);
}
