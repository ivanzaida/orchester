//! Flows the hub used to send to the old editor (`Pipelines · v2` P16 `T9KJFP`, P16b `P1853`; PV2-24): a step that
//! waits for only part of the stage before it is drawn under that stage with a *waits for … only* line on its card, and
//! its *Waits for* row in the step's *Run* group names and changes what it waits for; the card's ⋯ offers *Wait for
//! some steps only…* to every other step after the first stage. Steps with no order to draw (P16c) are
//! [`super::pipeline_unordered`]'s.
//! `cargo test -p orchester-app --test agents pipeline_waits`.

use std::time::{Duration, Instant};

use orchester_app::ui::tooltip::TOOLTIP_DELAY;
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, PipelineEdit, RepositoryAccess, StepKey},
};

use super::{
  harness::Agents,
  pipeline_flow::{add, agent, fails_back_to, open, store},
  pipeline_reorder::{close_menu, hover, open_menu},
  pipeline_step::{SAVE, open_step, save_until, stored},
};

const ROW: &str = "pipeline-hub-step-waits-row";
const ADD: &str = "pipeline-hub-step-waits-add";
const MENU: &str = "pipeline-hub-step-waits-menu";

/// P16's shape: research → plan → implement → { checks ∥ review } → notes, notes waiting for checks only.
struct Branched {
  pipeline: ResourceId,
  research: StepKey,
  implement: StepKey,
  checks: StepKey,
  review: StepKey,
  notes: StepKey,
}

fn branched(agents: &Agents) -> Branched {
  let builder = agents.profile(true);
  let reads = || agent(Some(builder), RepositoryAccess::ReadOnly);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let research = add(&mut edit, "Research", &[], reads());
  let plan = add(&mut edit, "Plan", &[&research], reads());
  let implement = add(&mut edit, "Implement", &[&plan], reads());
  let checks = add(&mut edit, "Run checks", &[&implement], reads());
  let review = add(&mut edit, "Review", &[&implement], reads());
  let notes = add(&mut edit, "Write notes", &[&checks], reads());
  let pipeline = store(agents, "Feature delivery", &edit.validate().expect("a pipeline"));
  Branched {
    pipeline,
    research,
    implement,
    checks,
    review,
    notes,
  }
}

fn chip(key: &StepKey) -> String {
  format!("pipeline-hub-step-waits-chip-{key}")
}

/// The name on step `key`'s chip.
fn name(key: &StepKey) -> String {
  format!("{}-name", chip(key))
}

fn choice(key: &StepKey) -> String {
  format!("pipeline-hub-step-waits-choice-{key}")
}

fn waits_line(key: &StepKey) -> String {
  format!("pipeline-hub-flow-step-{key}-waits-words")
}

/// What step `key` of the latest version waits for, sorted.
fn stored_waits(agents: &Agents, pipeline: ResourceId, key: &StepKey) -> Vec<String> {
  let body = stored(agents, pipeline);
  let mut waits: Vec<String> = body
    .step(key)
    .expect("the step")
    .depends_on
    .iter()
    .map(|key| key.as_str().to_owned())
    .collect();
  waits.sort();
  waits
}

/// *Back to …* from the open step to the pipeline's flow, which keeps the edit.
fn back(agents: &mut Agents) {
  agents.click("pipeline-hub-step-back");
  agents.until("the flow", |s| s.exists("pipeline-hub-flow"));
}

fn open_add(agents: &mut Agents) {
  agents.click(ADD);
  agents.until("the steps it may wait for", |s| s.exists(MENU));
}

#[test]
fn a_step_that_waits_for_part_of_a_group_stands_under_it_and_says_what_it_waits_for() {
  let mut agents = Agents::new(true);
  let flow = branched(&agents);
  open(&mut agents, flow.pipeline);

  assert!(!agents.exists("pipeline-hub-flow-undrawable"), "the steps are drawn");
  assert_eq!(
    agents.text(&waits_line(&flow.notes)),
    "waits for 04 Run checks only · starts before 05 Review ends"
  );
  for key in [&flow.research, &flow.checks, &flow.review] {
    assert!(
      !agents.exists(&waits_line(key)),
      "{key} waits for all of the stage before it"
    );
  }
}

#[test]
fn the_step_shows_waits_for_with_a_chip_per_step_it_names() {
  let mut agents = Agents::new(true);
  let flow = branched(&agents);
  open_step(&mut agents, flow.pipeline, &flow.notes);

  assert!(
    agents.exists(ROW),
    "Waits for shows for a step that waits for part of the stage"
  );
  assert!(agents.text(ROW).contains("Starts when all of these passed."));
  assert_eq!(agents.text(&name(&flow.checks)), "Run checks");
  assert!(!agents.exists(&chip(&flow.review)));

  open_step(&mut agents, flow.pipeline, &flow.review);
  assert!(
    !agents.exists(ROW),
    "a step that waits for the whole stage keeps P2 as it is"
  );
}

#[test]
fn add_offers_only_the_steps_that_do_not_wait_for_it_and_fades_those_it_waits_for_through_another() {
  let mut agents = Agents::new(true);
  let flow = branched(&agents);
  open_step(&mut agents, flow.pipeline, &flow.notes);
  open_add(&mut agents);

  for key in [&flow.research, &flow.implement, &flow.checks, &flow.review] {
    assert!(agents.exists(&choice(key)), "{key} is offered");
  }
  assert!(!agents.exists(&choice(&flow.notes)), "not the step itself");
  assert_eq!(
    agents.text(&format!("{}-through", choice(&flow.research))),
    "through 04"
  );
  agents.click(&choice(&flow.research));
  assert!(
    !agents.exists(&chip(&flow.research)),
    "a step waited for through another changes nothing"
  );

  open_step(&mut agents, flow.pipeline, &flow.checks);
  assert!(!agents.exists(ROW));
}

#[test]
fn ticking_the_rest_of_the_stage_takes_the_waits_line_away_and_keeps_the_row() {
  let mut agents = Agents::new(true);
  let flow = branched(&agents);
  open_step(&mut agents, flow.pipeline, &flow.notes);
  open_add(&mut agents);
  agents.click(&choice(&flow.review));
  agents.until("Review named", |s| s.exists(&chip(&flow.review)));
  assert!(agents.exists(MENU), "a tick keeps the menu open");
  assert!(agents.exists(ROW), "the row stays for the step it was changed for");

  back(&mut agents);
  assert!(
    !agents.exists(&waits_line(&flow.notes)),
    "Write notes waits for the whole stage now"
  );
  save_until(&mut agents, "saved", |s| {
    stored_waits(s, flow.pipeline, &flow.notes).len() == 2
  });
  assert_eq!(
    stored_waits(&agents, flow.pipeline, &flow.notes),
    [flow.checks.to_string(), flow.review.to_string()]
  );
}

#[test]
fn the_cross_stops_waiting_for_a_step_and_save_stores_a_step_that_waits_for_part_of_a_stage() {
  let mut agents = Agents::new(true);
  let flow = branched(&agents);
  open_step(&mut agents, flow.pipeline, &flow.notes);
  open_add(&mut agents);
  agents.click(&choice(&flow.review));
  agents.until("Review named", |s| s.exists(&chip(&flow.review)));
  agents.click(&format!("{}-remove", chip(&flow.checks)));
  agents.until("Run checks no longer named", |s| !s.exists(&chip(&flow.checks)));
  assert_eq!(agents.text(&name(&flow.review)), "Review");

  assert!(
    agents.exists(SAVE),
    "a step that waits for part of a stage can be saved"
  );
  save_until(&mut agents, "saved", |s| {
    stored_waits(s, flow.pipeline, &flow.notes) == [flow.review.to_string()]
  });
}

#[test]
fn wait_for_everything_before_it_puts_the_step_back_under_the_stage() {
  let mut agents = Agents::new(true);
  let flow = branched(&agents);
  open_step(&mut agents, flow.pipeline, &flow.notes);
  open_add(&mut agents);
  assert!(
    agents
      .text("pipeline-hub-step-waits-everything")
      .contains("Wait for everything before it")
  );
  agents.click("pipeline-hub-step-waits-everything");
  agents.until("both named", |s| {
    s.exists(&chip(&flow.checks)) && s.exists(&chip(&flow.review))
  });
  assert!(!agents.exists(MENU), "the choice closes the menu");
  back(&mut agents);
  assert!(!agents.exists(&waits_line(&flow.notes)));
}

#[test]
fn the_cards_menu_offers_wait_for_some_steps_only_and_opens_the_step_with_its_row() {
  let mut agents = Agents::new(true);
  let flow = branched(&agents);
  open(&mut agents, flow.pipeline);
  open_menu(&mut agents, &flow.research);
  assert!(
    !agents.exists("pipeline-hub-flow-menu-wait-some"),
    "nothing runs before the first stage"
  );
  close_menu(&mut agents);
  open_menu(&mut agents, &flow.notes);
  assert!(
    !agents.exists("pipeline-hub-flow-menu-wait-some"),
    "a step that waits for part of the stage has its row already"
  );
  close_menu(&mut agents);
  open_menu(&mut agents, &flow.review);
  assert!(
    agents
      .text("pipeline-hub-flow-menu-wait-some")
      .contains("Wait for some steps only…")
  );
  agents.click("pipeline-hub-flow-menu-wait-some");
  agents.until("the step with its row", |s| {
    s.exists("pipeline-hub-step-head") && s.exists(ROW)
  });
  assert_eq!(
    agents.text(&name(&flow.implement)),
    "Implement",
    "the stage before it as its chips"
  );
}

#[test]
fn the_open_steps_menu_adds_the_row_in_place() {
  let mut agents = Agents::new(true);
  let flow = branched(&agents);
  open_step(&mut agents, flow.pipeline, &flow.review);
  assert!(!agents.exists(ROW));
  agents.click("pipeline-hub-step-more");
  agents.until("the menu", |s| s.exists("pipeline-hub-flow-menu"));
  agents.click("pipeline-hub-flow-menu-wait-some");
  agents.until("the row", |s| s.exists(ROW));
  assert!(agents.exists(&chip(&flow.implement)));
}

#[test]
fn the_cross_of_a_step_a_failure_route_needs_changes_nothing() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let reads = || agent(Some(builder), RepositoryAccess::ReadOnly);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let plan = add(&mut edit, "Plan", &[], reads());
  let build = add(&mut edit, "Build", &[&plan], reads());
  let lint = add(&mut edit, "Lint", &[&plan], reads());
  let review = add(&mut edit, "Review", &[&build], reads());
  fails_back_to(&mut edit, &review, &build);
  let pipeline = store(&agents, "Guarded", &edit.validate().expect("a pipeline"));
  open_step(&mut agents, pipeline, &review);

  assert!(agents.exists(ROW), "Review waits for Build only, not Lint");
  agents.click(&format!("{}-remove", chip(&build)));
  assert!(
    agents.exists(&chip(&build)),
    "Review returns to Build on failure, so it keeps waiting for it"
  );
  assert!(!agents.exists(&chip(&lint)));
}

#[test]
fn the_refused_cross_says_why_where_the_pointer_rests_on_it() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let reads = || agent(Some(builder), RepositoryAccess::ReadOnly);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let plan = add(&mut edit, "Plan", &[], reads());
  let build = add(&mut edit, "Build", &[&plan], reads());
  add(&mut edit, "Lint", &[&plan], reads());
  let review = add(&mut edit, "Review", &[&build], reads());
  fails_back_to(&mut edit, &review, &build);
  let pipeline = store(&agents, "Guarded", &edit.validate().expect("a pipeline"));
  open_step(&mut agents, pipeline, &review);

  let cross = format!("{}-remove", chip(&build));
  let tooltip = format!("{cross}-tooltip");
  assert!(
    !agents.exists(&tooltip),
    "nothing is said before the pointer rests on it"
  );
  hover(&mut agents, &cross);
  agents
    .window
    .tree
    .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
  agents.window.pass();
  assert_eq!(agents.text(&tooltip), "04 Review returns to 02 Build on failure.");
}
