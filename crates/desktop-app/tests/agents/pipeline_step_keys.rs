//! Alt ↑ and Alt ↓ while a text field of the open step has the keys belong to the field (N-013): they move the caret
//! or select, never the step. `cargo test -p orchester-app --test agents pipeline_step_keys`.

use super::{
  harness::Agents,
  pipeline_reorder::alt,
  pipeline_step::{INSTRUCTIONS, chain, keys, open_step},
};

const POSITION: &str = "pipeline-hub-step-position";

/// Gives the keys to the element `id`, or takes them from it.
fn keys_on(agents: &mut Agents, id: &str, on: bool) {
  let mut element = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .unwrap_or_else(|| panic!("{id} exists"));
  if on {
    element.focus();
  } else {
    element.blur();
  }
  agents.window.pass();
}

#[test]
fn pipeline_step_keys_alt_up_in_the_instructions_does_not_move_the_step() {
  let mut agents = Agents::new(true);
  let pipeline = chain(&agents, 3);
  let all = keys(&agents, pipeline);
  open_step(&mut agents, pipeline, &all[1]);
  assert_eq!(agents.text(POSITION), "step 02 of 3");

  keys_on(&mut agents, INSTRUCTIONS, true);
  alt(&mut agents, "ArrowUp");
  alt(&mut agents, "ArrowDown");
  alt(&mut agents, "ArrowUp");
  for _ in 0..5 {
    agents.window.tree.tick_futures();
    agents.window.pass();
  }
  assert_eq!(
    agents.text(POSITION),
    "step 02 of 3",
    "the keys stay in the field: the step keeps its place"
  );
  assert!(
    !agents
      .text(&format!("pipeline-hub-row-{pipeline}-line"))
      .starts_with("Unsaved"),
    "nothing is held"
  );

  // Out of the field, the same keys move the step: the field was what kept them.
  keys_on(&mut agents, INSTRUCTIONS, false);
  alt(&mut agents, "ArrowUp");
  agents.until("moved up", |s| s.text(POSITION) == "step 01 of 3");
}
