//! The flow's polish of PV2-17: a step of a parallel group says in its menu that *Move up* and *Move down* take it out
//! of the group, a refused drop keeps saying why after it is let go until the next change, and a refusal of the keys
//! goes when the pointer leaves its card. `cargo test -p orchester-app --test agents pipeline_flow_polish`.

use super::{
  harness::Agents,
  pipeline_flow::open,
  pipeline_reorder::{alt, card, close_menu, flow, focus, hover, open_menu},
  pipeline_reorder_changes::{lift, on, release},
};

/// What a refusal under a card says.
const REFUSAL: &str = "pipeline-hub-flow-landing-parallel";

#[test]
fn pipeline_flow_polish_moving_a_step_of_a_group_says_it_leaves_the_group() {
  let mut agents = Agents::new(true);
  let (id, [_, _, build, _, review]) = flow(&agents, false);
  open(&mut agents, id);

  open_menu(&mut agents, &review);
  assert!(
    agents
      .text("pipeline-hub-flow-menu-up")
      .starts_with("Move up, out of the group"),
    "{}",
    agents.text("pipeline-hub-flow-menu-up")
  );
  assert!(
    agents
      .text("pipeline-hub-flow-menu-down")
      .starts_with("Move down, out of the group")
  );
  close_menu(&mut agents);

  open_menu(&mut agents, &build);
  let up = agents.text("pipeline-hub-flow-menu-up");
  assert!(
    up.starts_with("Move up") && !up.contains("group"),
    "a step on its own just moves: {up}"
  );
}

#[test]
fn pipeline_flow_polish_a_refused_drop_keeps_saying_why_until_the_next_change() {
  let mut agents = Agents::new(true);
  let (id, [research, plan, build, ..]) = flow(&agents, true);
  open(&mut agents, id);

  lift(&mut agents, &plan, |s| on(s, &research, 0.05));
  agents.until("the refusal while dragging", |s| {
    s.exists("pipeline-hub-flow-row-landing")
  });
  release(&mut agents);
  agents.until("let go", |s| !s.exists("pipeline-hub-flow-row-landing"));
  agents.until("the refusal stays", |s| s.exists(REFUSAL));
  assert_eq!(
    agents.text(REFUSAL),
    "Can't move Plan: it returns to 01 Research on failure."
  );

  // The pointer and the keys go elsewhere: it is still said.
  hover(&mut agents, &card(&build));
  focus(&mut agents, &research);
  agents.window.pass();
  assert!(
    agents.exists(REFUSAL),
    "the pointer leaving does not take a drop's refusal away"
  );

  // The next change does.
  focus(&mut agents, &build);
  alt(&mut agents, "ArrowDown");
  agents.until("gone with the next change", |s| !s.exists(REFUSAL));
}

#[test]
fn pipeline_flow_polish_a_refusal_of_the_keys_goes_when_the_pointer_leaves_its_card() {
  let mut agents = Agents::new(true);
  let (id, [research, plan, ..]) = flow(&agents, true);
  open(&mut agents, id);

  focus(&mut agents, &plan);
  alt(&mut agents, "ArrowUp");
  agents.until("the refusal", |s| s.exists(REFUSAL));
  // The pointer rests on the card and the keys go to another: the pointer keeps it said.
  hover(&mut agents, &card(&plan));
  focus(&mut agents, &research);
  agents.window.pass();
  assert!(agents.exists(REFUSAL), "it stays while the pointer is on its card");

  hover(&mut agents, &card(&research));
  agents.until("gone once the pointer left", |s| !s.exists(REFUSAL));
}
