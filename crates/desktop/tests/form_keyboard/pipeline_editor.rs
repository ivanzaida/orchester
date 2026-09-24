use lurq::node::element::ElementRef;

use crate::harness::{Desktop, expected};

const NAME: &str = "step-name";
const INSTRUCTIONS: &str = "step-instructions";
const BUDGET: &str = "step-budget";
const DESCRIPTION: &str = "pipeline-description";
/// More than the editor has stops, to find one by tabbing.
const MAX_STOPS: usize = 80;

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

#[test]
fn step_inspector_tab_follows_the_fields_and_skips_unavailable_ones() {
  let mut desktop = starter();
  let others = step_cards(&mut desktop) - 1;
  desktop.tab_to(NAME, MAX_STOPS);
  // Profile select; the model and Edit profile are unavailable without a profile and skipped;
  // instructions; one "runs after" toggle per other step and the route select; the budget
  // (the first step has no route, so no retries); repository access, approval, Duplicate
  // and Remove. Then Tab leaves the editor for the status bar.
  let mut order = vec![None, Some(INSTRUCTIONS)];
  order.extend(std::iter::repeat_n(None, others + 1));
  order.extend([
    Some(BUDGET),
    None,
    Some("step-approval"),
    Some("step-duplicate"),
    Some("step-remove"),
    Some("status-runner"),
  ]);
  assert_eq!(desktop.stops(order.len(), false), expected(&order));
  order.reverse();
  let back: Vec<Option<&str>> = order.into_iter().skip(1).chain([Some(NAME)]).collect();
  assert_eq!(desktop.stops(back.len(), true), expected(&back));
}

#[test]
fn toolbar_and_step_cards_are_tab_stops() {
  let mut desktop = starter();
  desktop.tab_to("pipeline-breadcrumb-library", MAX_STOPS);
  assert_eq!(
    desktop.stops(2, false),
    expected(&[Some("pipeline-add-step"), Some("pipeline-settings")])
  );
  let first = desktop.value(NAME);
  // Every card is a stop; Enter on another card selects its step.
  desktop.tab_to("step-card-plan", MAX_STOPS);
  desktop.enter();
  desktop.settle("the second step in the inspector", |tree| {
    tree
      .get_element_by_id_mut(NAME)
      .and_then(|element| element.as_text_input())
      .is_some_and(|input| input.value() != first)
  });
}

#[test]
fn step_inspector_remove_works_from_the_keyboard() {
  let mut desktop = starter();
  let cards = step_cards(&mut desktop);
  desktop.tab_to("step-remove", MAX_STOPS);
  // Remove asks once more.
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
  desktop.tab_to(NAME, MAX_STOPS);
  let name = desktop.value(NAME);
  desktop.enter();
  assert_eq!(desktop.value(NAME), name);
  assert_eq!(desktop.focused().as_deref(), Some(NAME));
}

#[test]
fn pipeline_inspector_description_is_reached_by_tab() {
  let mut desktop = starter();
  desktop.click("pipeline-settings");
  desktop.wait_for(DESCRIPTION);
  desktop.tab_to(DESCRIPTION, MAX_STOPS);
  desktop.type_text("Ships changes");
  assert!(desktop.value(DESCRIPTION).ends_with("Ships changes"));
}
