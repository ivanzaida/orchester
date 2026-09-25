//! A form control whose selected state is the 1.5px selection stroke (new pipeline option card)
//! shows no focus ring while selected, since stroke, gap and ring read as a double border. It
//! stays a Tab stop; unselected, it shows its ring when focused. lurq exposes no
//! resolved style of a node, so the ring is read from the rule the cards build their focused
//! style with, and the Tab stops from the tree.

use orchester_desktop::pressable::{Ring, stroke_selection_ring};

use crate::harness::Desktop;

/// More than the editor or the dialog has stops, to find one by tabbing.
const MAX_STOPS: usize = 80;

#[test]
fn stroke_selected_control_has_no_focus_ring_while_selected() {
  assert_eq!(stroke_selection_ring(Ring::Outer, true), None);
}

#[test]
fn stroke_selected_control_has_its_focus_ring_while_unselected() {
  assert_eq!(stroke_selection_ring(Ring::Outer, false), Some(Ring::Outer));
}

#[test]
fn chosen_start_option_stays_a_tab_stop() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-pipelines");
  desktop.click("nav-pipelines");
  desktop.wait_for("pipelines-new");
  desktop.click("pipelines-new");
  desktop.wait_for("new-pipeline-name");
  desktop.tab_to("new-pipeline-empty", MAX_STOPS);
  desktop.enter();
  desktop.tab_to("new-pipeline-empty", MAX_STOPS);
}
