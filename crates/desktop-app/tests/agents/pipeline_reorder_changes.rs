//! The changes a card's menu, the keys and the handle make to a pipeline's order, held by the panel until Save
//! (`Pipelines · v2` P3 `K4B56`, P4 `naN3G`; PV2-06): Alt ↑ / Alt ↓ with the focus kept, a refused move saying why, run
//! in parallel and on its own, add, duplicate and remove, and the drag of a handle with its line, its drop on a card
//! and Esc. `cargo test -p orchester-app --test agents pipeline_reorder`.

use lurq::app::events::MouseButton;
use orchester_core::{id::ResourceId, resource::StepKey};

use super::{
  harness::Agents,
  pipeline_flow::open,
  pipeline_reorder::{alt, card, close_menu, flow, focus, hover, open_menu, order, point, stored},
};

const UNSAVED: &str = "Unsaved changes";

fn y_of(agents: &mut Agents, key: &StepKey) -> f32 {
  agents
    .window
    .tree
    .get_element_by_id_mut(&card(key))
    .and_then(|found| found.bounds())
    .unwrap_or_else(|| panic!("{key} laid out"))
    .y
}

fn unsaved(agents: &mut Agents, id: ResourceId) -> bool {
  agents.text(&format!("pipeline-hub-row-{id}-line")).starts_with(UNSAVED)
}

fn key(name: &str) -> StepKey {
  StepKey::new(name).expect("a key")
}

#[test]
fn pipeline_reorder_alt_down_moves_the_card_the_keys_are_on_and_keeps_the_focus_for_the_next_key() {
  let mut agents = Agents::new(true);
  let (id, [research, plan, build, checks, review]) = flow(&agents, false);
  open(&mut agents, id);
  let before = stored(&agents, id);

  focus(&mut agents, &research);
  alt(&mut agents, "ArrowUp");
  assert!(
    !unsaved(&mut agents, id),
    "nothing runs before the first step, so nothing changes"
  );

  focus(&mut agents, &build);
  alt(&mut agents, "ArrowDown");
  agents.until("build after the group", |s| {
    order(s, &[build.clone(), checks.clone(), review.clone()])[2] == build
  });
  assert!(unsaved(&mut agents, id), "the edit is held");
  assert_eq!(stored(&agents, id), before, "nothing is stored until Save");

  // The card keeps the keys: the next Alt ↑ moves it back.
  agents.until("the card has the keys again", |s| {
    s.exists(&format!("{}-more", card(&build)))
  });
  alt(&mut agents, "ArrowUp");
  agents.until("build before the group again", |s| {
    order(s, &[build.clone(), checks.clone(), review.clone()])[0] == build
  });
  assert_eq!(order(&mut agents, &[research, plan]).len(), 2);

  agents.click("pipeline-hub-cancel");
  agents.until("put back", |s| !unsaved(s, id));
}

#[test]
fn pipeline_reorder_a_move_that_breaks_a_failure_route_says_why_and_changes_nothing() {
  let mut agents = Agents::new(true);
  let (id, [research, plan, ..]) = flow(&agents, true);
  open(&mut agents, id);
  let reason = "it returns to 01 Research on failure.";

  open_menu(&mut agents, &plan);
  assert_eq!(
    agents.text("pipeline-hub-flow-menu-up-detail"),
    reason,
    "the row is faded with its reason"
  );
  close_menu(&mut agents);

  focus(&mut agents, &plan);
  alt(&mut agents, "ArrowUp");
  agents.until("the reason", |s| s.exists("pipeline-hub-flow-landing-parallel"));
  assert_eq!(
    agents.text("pipeline-hub-flow-landing-parallel"),
    format!("Can't move Plan: {reason}")
  );
  assert!(!unsaved(&mut agents, id), "a refused move starts no edit");
  assert_eq!(order(&mut agents, &[research, plan.clone()])[1], plan);
}

#[test]
fn pipeline_reorder_run_in_parallel_and_run_on_its_own_from_the_menu() {
  let mut agents = Agents::new(true);
  let (id, [_, plan, build, ..]) = flow(&agents, false);
  open(&mut agents, id);

  open_menu(&mut agents, &build);
  agents.click("pipeline-hub-flow-menu-parallel-0");
  agents.until("side by side", |s| (y_of(s, &build) - y_of(s, &plan)).abs() < 1.0);
  assert!(unsaved(&mut agents, id));

  open_menu(&mut agents, &build);
  assert!(
    agents.exists("pipeline-hub-flow-menu-own"),
    "inside a group it can run on its own"
  );
  agents.click("pipeline-hub-flow-menu-own");
  agents.until("on its own", |s| y_of(s, &build) > y_of(s, &plan) + 1.0);
}

#[test]
fn pipeline_reorder_add_and_remove_go_through_the_edit() {
  let mut agents = Agents::new(true);
  let (id, [research, plan, _, checks, review]) = flow(&agents, false);
  open(&mut agents, id);
  let before = stored(&agents, id);
  let added = key("step-6");

  agents.click("pipeline-hub-add-step");
  agents.until("a sixth step at the end", |s| s.exists(&card(&added)));
  assert_eq!(agents.text("pipeline-hub-steps-count"), "6");
  assert!(
    y_of(&mut agents, &added) > y_of(&mut agents, &review),
    "after the last stage"
  );

  open_menu(&mut agents, &research);
  agents.click("pipeline-hub-flow-menu-add");
  let seventh = key("step-7");
  agents.until("a seventh step after research", |s| s.exists(&card(&seventh)));
  let between = y_of(&mut agents, &seventh);
  assert!(
    y_of(&mut agents, &research) < between && between < y_of(&mut agents, &plan),
    "in series, between research and plan"
  );

  open_menu(&mut agents, &checks);
  agents.click("pipeline-hub-flow-menu-remove");
  agents.until("checks gone", |s| !s.exists(&card(&checks)));
  assert_eq!(agents.text("pipeline-hub-steps-count"), "6");
  assert_eq!(stored(&agents, id), before, "nothing is stored until Save");
}

#[test]
fn pipeline_reorder_duplicate_runs_the_copy_beside_the_step() {
  let mut agents = Agents::new(true);
  let (id, [research, ..]) = flow(&agents, false);
  open(&mut agents, id);
  open_menu(&mut agents, &research);
  agents.click("pipeline-hub-flow-menu-duplicate");
  let copy = key("step-copy");
  agents.until("the copy", |s| s.exists(&card(&copy)));
  assert!(
    (y_of(&mut agents, &copy) - y_of(&mut agents, &research)).abs() < 1.0,
    "side by side"
  );
  assert!(agents.exists("pipeline-hub-flow-parallel-1"), "in a group of their own");
  assert!(
    !agents.exists("pipeline-hub-flow-undrawable"),
    "the flow still draws as one line"
  );
}

/// Presses the left button on `key`'s handle and moves to the point `to` gives, in steps; the card is lifted.
fn lift(agents: &mut Agents, key: &StepKey, to: impl FnOnce(&mut Agents) -> (f32, f32)) {
  hover(agents, &card(key));
  let handle = format!("{}-handle", card(key));
  agents.until("the handle", |s| s.exists(&handle));
  let bounds = agents
    .window
    .tree
    .get_element_by_id_mut(&handle)
    .and_then(|found| found.bounds())
    .expect("the handle");
  let (x, y) = point(agents, bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  agents.window.tree.mouse_move(x, y);
  agents.window.tree.mouse_down(x, y, MouseButton::Left);
  agents.window.pass();
  let (tx, ty) = to(agents);
  let (tx, ty) = point(agents, tx, ty);
  for step in 1..=8_u8 {
    let share = f32::from(step) / 8.0;
    agents
      .window
      .tree
      .mouse_move(x + (tx - x) * share, y + (ty - y) * share);
    agents.window.pass();
  }
}

fn release(agents: &mut Agents) {
  agents.window.tree.mouse_up(0.0, 0.0, MouseButton::Left);
  agents.window.pass();
}

/// The point at `share` of `key`'s card from its top, in the middle of it.
fn on(agents: &mut Agents, key: &StepKey, share: f32) -> (f32, f32) {
  let bounds = agents
    .window
    .tree
    .get_element_by_id_mut(&card(key))
    .and_then(|found| found.bounds())
    .expect("the card");
  (bounds.x + bounds.width / 2.0, bounds.y + bounds.height * share)
}

#[test]
fn pipeline_reorder_dragging_a_handle_shows_the_line_and_what_it_means_and_a_drop_lands_there() {
  let mut agents = Agents::new(true);
  let (id, [research, plan, _, _, review]) = flow(&agents, false);
  open(&mut agents, id);
  let before = stored(&agents, id);

  lift(&mut agents, &review, |s| on(s, &plan, 0.05));
  agents.until("the line", |s| s.exists("pipeline-hub-flow-row-landing"));
  assert_eq!(
    agents.text("pipeline-hub-flow-landing"),
    "05 Review goes here runs after 01 Research, before 02 Plan"
  );
  assert!(
    agents.exists("pipeline-hub-flow-lifted"),
    "the card follows the pointer"
  );
  assert_eq!(stored(&agents, id), before);

  release(&mut agents);
  agents.until("dropped", |s| !s.exists("pipeline-hub-flow-row-landing"));
  assert!(!agents.exists("pipeline-hub-flow-lifted"));
  let (r, v, p) = (
    y_of(&mut agents, &research),
    y_of(&mut agents, &review),
    y_of(&mut agents, &plan),
  );
  assert!(r < v && v < p, "review stands between research and plan: {r} {v} {p}");
  assert!(unsaved(&mut agents, id));
}

#[test]
fn pipeline_reorder_dropping_on_a_card_runs_the_two_in_parallel() {
  let mut agents = Agents::new(true);
  let (id, [_, plan, _, _, review]) = flow(&agents, false);
  open(&mut agents, id);

  lift(&mut agents, &review, |s| on(s, &plan, 0.5));
  agents.until("the words", |s| s.exists("pipeline-hub-flow-landing-parallel"));
  assert_eq!(
    agents.text("pipeline-hub-flow-landing-parallel"),
    "05 Review runs in parallel with 02 Plan"
  );
  release(&mut agents);
  agents.until("side by side", |s| (y_of(s, &review) - y_of(s, &plan)).abs() < 1.0);
  assert!(unsaved(&mut agents, id));
}

#[test]
fn pipeline_reorder_escape_sets_a_lifted_card_down_and_nothing_changes() {
  let mut agents = Agents::new(true);
  let (id, [_, plan, _, _, review]) = flow(&agents, false);
  open(&mut agents, id);
  let at = y_of(&mut agents, &review);

  lift(&mut agents, &review, |s| on(s, &plan, 0.05));
  agents.until("the line", |s| s.exists("pipeline-hub-flow-row-landing"));
  agents
    .window
    .tree
    .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  agents.until("set down", |s| !s.exists("pipeline-hub-flow-row-landing"));
  release(&mut agents);
  assert!(!agents.exists("pipeline-hub-flow-lifted"));
  assert!(
    (y_of(&mut agents, &review) - at).abs() < 1.0,
    "the card is where it was"
  );
  assert!(!unsaved(&mut agents, id));
}

#[test]
fn pipeline_reorder_a_drag_over_a_place_that_breaks_a_failure_route_says_why_and_drops_nothing() {
  let mut agents = Agents::new(true);
  let (id, [research, plan, ..]) = flow(&agents, true);
  open(&mut agents, id);

  lift(&mut agents, &plan, |s| on(s, &research, 0.05));
  agents.until("the line", |s| s.exists("pipeline-hub-flow-row-landing"));
  assert_eq!(
    agents.text("pipeline-hub-flow-landing"),
    "Can't move Plan: it returns to 01 Research on failure."
  );
  release(&mut agents);
  agents.until("set down", |s| !s.exists("pipeline-hub-flow-row-landing"));
  assert!(!unsaved(&mut agents, id), "a refused drop changes nothing");
}
