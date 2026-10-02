//! How the approval dialog lays out long values and its footer: Copy on the label's line, the box
//! across the full width, a command after its working directory, and Deny and Approve once kept
//! whole when a decision fails.

use crate::{
  github::shows,
  harness::Desktop,
  tool_approval::{ACTION, ARGUMENTS, DIALOG, HEREDOC, TARGET, heredoc, large_arguments, long_target},
};

/// Copy of `id` sits on its label's line above the box, at the box's right edge, and the lines use
/// the box's full width.
fn assert_copy_beside_the_label(desktop: &mut Desktop, id: &str) {
  let frame = desktop.bounds(id);
  let viewport = desktop.bounds(&format!("{id}-scroll"));
  let copy = desktop.bounds(&format!("{id}-copy"));
  assert!(copy.y + copy.height <= frame.y, "{id}'s Copy is above its box");
  assert!(
    (copy.x + copy.width - (frame.x + frame.width)).abs() < 0.5,
    "{id}'s Copy ends where the box does"
  );
  // Copy is the ghost button of `feLwP`: 22 high, about 59 wide.
  assert!((copy.height - 22.0).abs() < 0.5, "{id}'s Copy is {} high", copy.height);
  assert!(copy.width < 64.0, "{id}'s Copy is {} wide", copy.width);
  // The viewport spans the box but for its 6px right padding; its vertical scrollbar has a lane of
  // its own inside it.
  assert!(
    viewport.width >= frame.width - 8.0,
    "{id}'s lines take {} of {} px",
    viewport.width,
    frame.width
  );
  // 12 between the label's line and the box (`E5NBo`).
  let gap = frame.y - (copy.y + copy.height);
  assert!((gap - 12.0).abs() < 0.5, "{id} is {gap} px under its label");
}

#[test]
fn tool_approval_copy_sits_beside_the_label_and_the_lines_use_the_full_width() {
  let mut desktop = heredoc();
  assert_copy_beside_the_label(&mut desktop, ACTION);
  let mut desktop = large_arguments();
  assert_copy_beside_the_label(&mut desktop, ARGUMENTS);
}

/// Deny and Approve once: both inside the dialog, at their own width.
fn assert_buttons_whole(desktop: &mut Desktop, widths: Option<[f32; 2]>) -> [f32; 2] {
  let dialog = desktop.bounds(DIALOG);
  let mut measured = [0.0; 2];
  for (index, button) in ["tool-approval-deny", "tool-approval-approve-once"]
    .into_iter()
    .enumerate()
  {
    let bounds = desktop.bounds(button);
    assert!(
      bounds.x >= dialog.x && bounds.x + bounds.width <= dialog.x + dialog.width + 0.5,
      "{button} is inside the dialog: {bounds:?} in {dialog:?}"
    );
    if let Some(widths) = widths {
      assert!((bounds.width - widths[index]).abs() < 0.5, "{button} keeps its width");
    }
    measured[index] = bounds.width;
  }
  measured
}

#[test]
fn tool_approval_runner_offline_keeps_both_buttons_in_the_dialog() {
  for width in [1440, 960] {
    let mut desktop = large_arguments();
    desktop.tree.resize(width, 960);
    desktop.pass();
    let widths = assert_buttons_whole(&mut desktop, None);
    // No runner runs in this harness: the decision cannot reach it.
    desktop.press("tool-approval-deny");
    desktop.wait_until("the reason the decision failed", |desktop| {
      shows(desktop, "tool-approval-actions", "The runner is not connected")
    });
    assert_buttons_whole(&mut desktop, Some(widths));
  }
}

/// The ids and texts of `element` and everything in it, in reading order.
fn reading_order(element: lurq::node::element::ElementRef<'_>, out: &mut Vec<String>) {
  if let Some(id) = element.id() {
    out.push(id.to_owned());
  }
  if let Some(text) = element.text_content() {
    out.push(text.to_owned());
  }
  for child in element.children() {
    reading_order(child, out);
  }
}

#[test]
fn tool_approval_command_comes_after_its_folder_under_a_label_that_counts_its_lines() {
  let mut desktop = heredoc();
  let lines = HEREDOC.split('\n').count();
  assert!(shows(
    &mut desktop,
    DIALOG,
    &format!("Command · {lines} lines, as recorded")
  ));
  let mut order = Vec::new();
  reading_order(desktop.tree.get_element_by_id(DIALOG).expect("dialog"), &mut order);
  let at = |item: &str| {
    order
      .iter()
      .position(|entry| entry == item)
      .unwrap_or_else(|| panic!("{item} is shown"))
  };
  assert!(at("Agent") < at("Working directory"));
  assert!(at("Working directory") < at(ACTION), "the command box comes last");
  assert!(
    !order.iter().any(|entry| entry == "Shell"),
    "a CLI's request names no shell"
  );
  // A single-line target sits in a box 44 high: 6 + 4, one line, 4 + the 6px lane + 6.
  let mut desktop = long_target();
  let height = desktop.bounds(TARGET).height;
  assert!((height - 44.0).abs() < 1.0, "the target's box is {height} px high");
}

#[test]
fn tool_approval_copy_hover_after_a_resize_keeps_the_backdrop_on_the_window() {
  // Hovering Copy renders the dialog again; after two resizes lurq used to lay its layer out at the
  // first resize's size.
  let mut desktop = large_arguments();
  desktop.tree.resize(960, 700);
  desktop.pass();
  desktop.tree.resize(1440, 960);
  desktop.pass();
  let before = desktop.bounds("dialog-backdrop");
  desktop.hover(&format!("{ARGUMENTS}-copy"));
  desktop.pass();
  let after = desktop.bounds("dialog-backdrop");
  assert_eq!(
    (after.x, after.y, after.width, after.height),
    (before.x, before.y, before.width, before.height),
    "the backdrop keeps the window's size after Copy is hovered"
  );
  assert!(
    (after.width - 1440.0).abs() < 0.5,
    "the backdrop is {} wide",
    after.width
  );
}
