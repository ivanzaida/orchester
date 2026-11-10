//! What the flow keeps from breaking or losing a change (PV2-06 round 2): a step another step returns to cannot be
//! removed, a step added inside a parallel group waits for the whole group, the flow is locked while *Allow these
//! changes?* is open, a drop outside the flow drops nothing, a copy is named, and a reorder that crosses no approval or
//! tool step saves without the question. `cargo test -p orchester-app --test agents pipeline_reorder`.

use lurq::app::events::MouseButton;
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, PipelineEdit, RepositoryAccess, StepKey},
};

use super::{
  harness::Agents,
  pipeline_flow::{add, agent, open, store},
  pipeline_reorder::{alt, card, flow, focus, hover, open_menu, point, stored},
};

fn key(name: &str) -> StepKey {
  StepKey::new(name).expect("a key")
}

fn y_of(agents: &mut Agents, id: &str) -> f32 {
  agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|found| found.bounds())
    .unwrap_or_else(|| panic!("{id} laid out"))
    .y
}

fn unsaved(agents: &mut Agents, id: ResourceId) -> bool {
  agents
    .text(&format!("pipeline-hub-row-{id}-line"))
    .starts_with("Unsaved changes")
}

/// research → { a ∥ b } → last: a group with a stage after it.
fn grouped(agents: &Agents) -> (ResourceId, [StepKey; 4]) {
  let profile = Some(agents.profile(true));
  let kind = || agent(profile, RepositoryAccess::ReadOnly);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let research = add(&mut edit, "Research", &[], kind());
  let a = add(&mut edit, "Alpha", &[&research], kind());
  let b = add(&mut edit, "Beta", &[&research], kind());
  let last = add(&mut edit, "Last", &[&a, &b], kind());
  let body = edit.validate().expect("a pipeline");
  (store(agents, "Grouped", &body, true), [research, a, b, last])
}

/// research → plan (gated) → build.
fn gated(agents: &Agents) -> (ResourceId, [StepKey; 3]) {
  let profile = Some(agents.profile(true));
  let kind = || agent(profile, RepositoryAccess::ReadOnly);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let research = add(&mut edit, "Research", &[], kind());
  let plan = add(&mut edit, "Plan", &[&research], kind());
  edit.update(&plan, |step| step.approval_gate = true);
  let build = add(&mut edit, "Build", &[&plan], kind());
  let body = edit.validate().expect("a pipeline");
  (store(agents, "Gated", &body, true), [research, plan, build])
}

#[test]
fn pipeline_reorder_a_step_another_step_returns_to_cannot_be_removed_and_says_why() {
  let mut agents = Agents::new(true);
  let (id, [research, ..]) = flow(&agents, true);
  open(&mut agents, id);

  open_menu(&mut agents, &research);
  assert_eq!(
    agents.text("pipeline-hub-flow-menu-remove-detail"),
    "02 Plan returns to 01 Research on failure, so 01 Research can't be removed."
  );
  agents.click("pipeline-hub-flow-menu-remove");
  assert!(agents.exists(&card(&research)), "the faded row removes nothing");
  assert!(!unsaved(&mut agents, id));
  assert!(
    !agents.exists("pipeline-hub-flow-undrawable"),
    "the flow still draws as one line"
  );
}

#[test]
fn pipeline_reorder_removing_a_step_of_a_group_keeps_the_flow_drawn() {
  let mut agents = Agents::new(true);
  let (id, [_, a, b, last]) = grouped(&agents);
  open(&mut agents, id);

  open_menu(&mut agents, &a);
  agents.click("pipeline-hub-flow-menu-remove");
  agents.until("alpha gone", |s| !s.exists(&card(&a)));
  assert!(!agents.exists("pipeline-hub-flow-undrawable"));
  assert!(y_of(&mut agents, &card(&last)) > y_of(&mut agents, &card(&b)));
  assert_eq!(agents.text("pipeline-hub-steps-count"), "3");
}

#[test]
fn pipeline_reorder_adding_after_a_step_of_a_group_waits_for_the_whole_group() {
  let mut agents = Agents::new(true);
  let (id, [_, a, b, last]) = grouped(&agents);
  open(&mut agents, id);

  open_menu(&mut agents, &a);
  agents.click("pipeline-hub-flow-menu-add");
  let added = key("step-5");
  agents.until("the new step", |s| s.exists(&card(&added)));
  assert!(
    !agents.exists("pipeline-hub-flow-undrawable"),
    "the flow still draws as one line"
  );
  let (group, new, after) = (
    y_of(&mut agents, &card(&b)).max(y_of(&mut agents, &card(&a))),
    y_of(&mut agents, &card(&added)),
    y_of(&mut agents, &card(&last)),
  );
  assert!(
    group < new && new < after,
    "in series after the group, before the last step"
  );
}

#[test]
fn pipeline_reorder_adding_after_a_step_of_a_last_group_waits_for_the_whole_group() {
  let mut agents = Agents::new(true);
  let (id, [.., review]) = flow(&agents, false);
  let checks = key("step-4");
  open(&mut agents, id);

  open_menu(&mut agents, &review);
  agents.click("pipeline-hub-flow-menu-add");
  let added = key("step-6");
  agents.until("the new step", |s| s.exists(&card(&added)));
  assert!(!agents.exists("pipeline-hub-flow-undrawable"));
  assert!(
    y_of(&mut agents, &card(&added)) > y_of(&mut agents, &card(&checks)),
    "after both steps of the group"
  );
}

#[test]
fn pipeline_reorder_the_flow_is_locked_while_the_question_about_a_save_is_open() {
  let mut agents = Agents::new(true);
  let (id, [research, plan, build]) = gated(&agents);
  open(&mut agents, id);
  let before = stored(&agents, id);

  // Build before the gated plan stops waiting for an approval: the save asks.
  open_menu(&mut agents, &build);
  agents.click("pipeline-hub-flow-menu-up");
  agents.press_until("pipeline-hub-save", "the question", |s| {
    s.exists("pipeline-hub-confirm-confirm")
  });

  hover(&mut agents, &card(&plan));
  agents.window.pass();
  assert!(
    !agents.exists(&format!("{}-handle", card(&plan))) && !agents.exists(&format!("{}-more", card(&plan))),
    "no handle and no ⋯ while the question is open"
  );
  focus(&mut agents, &research);
  let (research_y, plan_y) = (y_of(&mut agents, &card(&research)), y_of(&mut agents, &card(&plan)));
  alt(&mut agents, "ArrowDown");
  assert_eq!(y_of(&mut agents, &card(&research)), research_y, "the keys move nothing");
  assert_eq!(y_of(&mut agents, &card(&plan)), plan_y);
  let count = agents.text("pipeline-hub-steps-count");
  agents.click("pipeline-hub-add-step");
  assert_eq!(agents.text("pipeline-hub-steps-count"), count, "Add step is faded");
  assert_eq!(stored(&agents, id), before);

  // Keep editing gives the flow back.
  agents.click("pipeline-hub-confirm-keep");
  agents.until("unlocked", |s| {
    hover(s, &card(&plan));
    s.exists(&format!("{}-handle", card(&plan)))
  });
}

#[test]
fn pipeline_reorder_dropping_outside_the_flow_drops_nothing() {
  let mut agents = Agents::new(true);
  let (id, [.., review]) = flow(&agents, false);
  open(&mut agents, id);
  let frame = agents
    .window
    .tree
    .get_element_by_id_mut("pipeline-hub-flow-frame")
    .and_then(|found| found.bounds())
    .expect("the flow's frame");

  for target_y in [frame.y - 24.0, frame.y + frame.height + 140.0] {
    hover(&mut agents, &card(&review));
    let handle = format!("{}-handle", card(&review));
    agents.until("the handle", |s| s.exists(&handle));
    let found = agents
      .window
      .tree
      .get_element_by_id_mut(&handle)
      .and_then(|found| found.bounds())
      .expect("the handle");
    let (x, y) = point(&mut agents, found.x + found.width / 2.0, found.y + found.height / 2.0);
    agents.window.tree.mouse_move(x, y);
    agents.window.tree.mouse_down(x, y, MouseButton::Left);
    agents.window.pass();
    let (tx, ty) = point(&mut agents, found.x + found.width / 2.0, target_y);
    for step in 1..=8_u8 {
      let share = f32::from(step) / 8.0;
      agents
        .window
        .tree
        .mouse_move(x + (tx - x) * share, y + (ty - y) * share);
      agents.window.pass();
    }
    assert!(
      !agents.exists("pipeline-hub-flow-row-landing") && !agents.exists("pipeline-hub-flow-landing-parallel"),
      "nothing to land on outside the flow (y {target_y})"
    );
    agents.window.tree.mouse_up(tx, ty, MouseButton::Left);
    agents.window.pass();
    assert!(!unsaved(&mut agents, id), "nothing was dropped");
  }
}

#[test]
fn pipeline_reorder_a_copy_is_named_after_the_step() {
  let mut agents = Agents::new(true);
  let (id, [research, ..]) = flow(&agents, false);
  open(&mut agents, id);
  open_menu(&mut agents, &research);
  agents.click("pipeline-hub-flow-menu-duplicate");
  let copy = key("step-copy");
  agents.until("the copy", |s| s.exists(&card(&copy)));
  assert_eq!(agents.text(&format!("{}-name", card(&copy))), "Research copy");
  assert_eq!(agents.text(&format!("{}-name", card(&research))), "Research");
}

#[test]
fn pipeline_reorder_a_reorder_that_crosses_an_approval_gate_asks_before_it_saves() {
  let mut agents = Agents::new(true);
  let (id, [_, _, build]) = gated(&agents);
  open(&mut agents, id);
  let before = stored(&agents, id);
  open_menu(&mut agents, &build);
  agents.click("pipeline-hub-flow-menu-up");
  agents.press_until("pipeline-hub-save", "the question", |s| {
    s.exists("pipeline-hub-confirm-confirm")
  });
  assert_eq!(stored(&agents, id), before, "nothing is saved until Allow");
  agents.press_until("pipeline-hub-confirm-confirm", "saved", |s| stored(s, id) != before);
}
