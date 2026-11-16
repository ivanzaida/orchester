//! The strip of an open step scrolls at once (PV2-07, fixed with PV2-06): Shift+wheel (which the shell delivers as a
//! sideways delta) and the arrows move the chips in the same pass, with no pointer move after them to make the window
//! draw; a wheel that only goes up or down is left to the panel. `cargo test -p orchester-app --test agents
//! pipeline_step_scroll`.

use lurq::app::events::ScrollPhase;

use super::{
  harness::Agents,
  pipeline_step::{chain, keys, open_step, rect},
};

/// A wheel or swipe over the strip's middle, `delta_x` and `delta_y` in logical pixels.
fn wheel(agents: &mut Agents, delta_x: f32, delta_y: f32) {
  let (x, y) = agents
    .window
    .tree
    .get_element_by_id_mut("pipeline-hub-step-scroll")
    .and_then(|element| element.bounds())
    .expect("the strip")
    .center();
  let scale = agents.window.tree.scale_factor();
  let (x, y) = (x * scale, y * scale);
  agents.window.tree.mouse_move(x, y);
  agents
    .window
    .tree
    .scroll(x, y, delta_x * scale, delta_y * scale, ScrollPhase::Scroll);
  agents.window.pass();
}

/// The strip of a long pipeline opened on its last step, and that step's chip.
fn long_strip() -> (Agents, String) {
  let mut agents = Agents::new(true);
  let long = chain(&agents, 16);
  let last = keys(&agents, long).pop().expect("steps");
  open_step(&mut agents, long, &last);
  agents.until("scrolled to the end", |s| {
    s.exists("pipeline-hub-step-strip-before") && !s.exists("pipeline-hub-step-strip-after")
  });
  (agents, format!("pipeline-hub-step-chip-{last}"))
}

#[test]
fn pipeline_step_scroll_a_wheel_that_only_goes_up_or_down_is_left_to_the_panel() {
  // lurq hands the strip no such wheel (its handler is never called), so the panel around it scrolls instead.
  let (mut agents, chip) = long_strip();
  let start = rect(&mut agents, &chip).x;

  wheel(&mut agents, 0.0, 120.0);
  assert_eq!(rect(&mut agents, &chip).x, start, "the strip stays where it is");
}

#[test]
fn pipeline_step_scroll_shift_wheel_moves_the_strip_at_once() {
  let (mut agents, chip) = long_strip();
  let start = rect(&mut agents, &chip).x;

  // Shift+wheel arrives as a sideways delta with none up or down; the strip is at its end, so it can only go back.
  wheel(&mut agents, 90.0, 0.0);
  let moved = rect(&mut agents, &chip).x - start;
  assert!(
    (moved - 90.0).abs() < 1.0,
    "a sideways wheel moved the chips in the same pass: {moved}"
  );

  wheel(&mut agents, -90.0, 0.0);
  let back = rect(&mut agents, &chip).x - start;
  assert!(back.abs() < 1.0, "and the other way brought them back at once: {back}");
}

#[test]
fn pipeline_step_scroll_the_arrows_move_the_strip_at_once() {
  let (mut agents, chip) = long_strip();
  let start = rect(&mut agents, &chip).x;

  agents.click("pipeline-hub-step-strip-before");
  let moved = rect(&mut agents, &chip).x - start;
  assert!(moved > 50.0, "‹ moved the chips in the press's own pass: {moved}");
  assert!(agents.exists("pipeline-hub-step-strip-after"), "and › shows");

  let after = rect(&mut agents, &chip).x;
  agents.click("pipeline-hub-step-strip-after");
  let back = rect(&mut agents, &chip).x;
  assert!(
    back < after - 50.0,
    "› moved them back in its own pass: {after} then {back}"
  );
}
