use lurq::node::element::ElementRef;

use crate::harness::{Desktop, expected};

const NAME: &str = "step-name";
const INSTRUCTIONS: &str = "step-instructions";
const RETRIES: &str = "step-retries";
const BUDGET: &str = "step-budget";
const DESCRIPTION: &str = "pipeline-description";
const INPUTS: [&str; 5] = [NAME, INSTRUCTIONS, RETRIES, BUDGET, DESCRIPTION];
/// More than the editor has stops, to find one by tabbing.
const MAX_STOPS: usize = 60;

/// The starter pipeline open in the editor with its first step selected. The project has no
/// profiles, so no step has one and the step's model is unavailable.
fn starter() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-pipelines");
  desktop.click("nav-pipelines");
  desktop.wait_for("pipelines-create-starter");
  desktop.click("pipelines-create-starter");
  desktop.wait_for(NAME);
  desktop
}

fn count_step_cards(element: ElementRef<'_>) -> usize {
  let own = usize::from(element.id().is_some_and(|id| id.starts_with("step-card-")));
  own + element.children().into_iter().map(count_step_cards).sum::<usize>()
}

fn step_cards(desktop: &mut Desktop) -> usize {
  desktop.tree.root().map_or(0, count_step_cards)
}

/// Tabs until `input` has the focus.
fn tab_to(desktop: &mut Desktop, input: &str) {
  let reached = desktop
    .stops(MAX_STOPS, false, &INPUTS)
    .iter()
    .position(|stop| stop.as_deref() == Some(input));
  assert!(reached.is_some(), "Tab reaches {input}");
  let stops = reached.expect("checked above") + 1;
  // `stops` pressed Tab MAX_STOPS times; go back to the stop that reached the input.
  desktop.stops(MAX_STOPS - stops, true, &INPUTS);
}

#[test]
fn step_inspector_tab_follows_the_fields_and_skips_unavailable_ones() {
  let mut desktop = starter();
  let others = step_cards(&mut desktop) - 1;
  tab_to(&mut desktop, NAME);
  // Profile select; the model and Edit profile are unavailable without a profile and skipped;
  // instructions; one "runs after" toggle per other step and the route select; the budget
  // (the first step has no route, so no retries); repository access, approval, Duplicate
  // and Remove.
  let mut order = vec![None, Some(INSTRUCTIONS)];
  order.extend(std::iter::repeat_n(None, others + 1));
  order.push(Some(BUDGET));
  order.extend([None; 4]);
  assert_eq!(desktop.stops(order.len(), false, &INPUTS), expected(&order));
  order.reverse();
  let back: Vec<Option<&str>> = order.into_iter().skip(1).chain([Some(NAME)]).collect();
  assert_eq!(desktop.stops(back.len(), true, &INPUTS), expected(&back));
}

#[test]
fn step_inspector_remove_works_from_the_keyboard() {
  let mut desktop = starter();
  let cards = step_cards(&mut desktop);
  tab_to(&mut desktop, BUDGET);
  // Repository access, approval, Duplicate, Remove; Remove asks once more.
  desktop.stops(4, false, &INPUTS);
  desktop.enter();
  desktop.enter();
  // Removing is an edit of the editor state, shown by the next render. Background work is
  // not polled here: the probes' edits left draft saves running, and polling their
  // completion hangs (a defect of the editor's saving, reported separately).
  desktop.tree.rebuild();
  assert_eq!(step_cards(&mut desktop), cards - 1);
}

#[test]
fn step_inspector_enter_in_the_name_submits_nothing() {
  let mut desktop = starter();
  tab_to(&mut desktop, NAME);
  let name = desktop.value(NAME);
  desktop.enter();
  assert_eq!(desktop.value(NAME), name);
  assert_eq!(desktop.focused_input(&INPUTS).as_deref(), Some(NAME));
}

#[test]
fn pipeline_inspector_description_is_reached_by_tab() {
  let mut desktop = starter();
  desktop.click("pipeline-settings");
  desktop.wait_for(DESCRIPTION);
  tab_to(&mut desktop, DESCRIPTION);
  desktop.type_text("Ships changes");
  assert!(desktop.value(DESCRIPTION).ends_with("Ships changes"));
}
