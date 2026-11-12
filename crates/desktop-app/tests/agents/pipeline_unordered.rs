//! Steps with no order to draw in the Pipelines hub (`Pipelines · v2` P16c `YHAyx`; PV2-24): the warning notice says
//! why for each kind of problem and offers *Make it a line* when a line puts the steps right, as *Ready?* does; the
//! line draws the flow at once and *Save* stores it. The hub never makes such steps and a version cannot hold them, so
//! they are put into the panel's edit through the test-support slot (`orchester_app::test_support::pipeline_edit`).
//! `cargo test -p orchester-app --test agents pipeline_unordered`.

use orchester_app::test_support::pipeline_edit::{self, arm};
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, PipelineEdit, RepositoryAccess, StepKey},
};

use super::{
  harness::Agents,
  pipeline_flow::{add, agent, open, store},
  pipeline_step::{save_until, stored},
};

const NOTICE: &str = "pipeline-hub-flow-undrawable";
const LINE: &str = "pipeline-hub-flow-undrawable-line";
const READY_LINE: &str = "team-hub-ready-steps-make-line";
const HINT: &str = "team-hub-actions-hint";
const MAKE_A_LINE: &str =
  "Make it a line to give the steps an order: each step then waits for the step listed before it.";

/// Steps run one after the other, named `names`, each running the profile; their keys in order.
fn chain(agents: &Agents, names: &[&str]) -> (ResourceId, Vec<StepKey>) {
  let builder = agents.profile(true);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let mut keys: Vec<StepKey> = Vec::new();
  for name in names {
    let after: Vec<&StepKey> = keys.last().into_iter().collect();
    keys.push(add(
      &mut edit,
      name,
      &after,
      agent(Some(builder), RepositoryAccess::ReadOnly),
    ));
  }
  (store(agents, "Unordered", &edit.validate().expect("a pipeline")), keys)
}

/// P16c's shape: research → plan → implement → { checks ∥ review } → notes, with review made to wait for notes too.
fn review_and_notes_wait_for_each_other(agents: &mut Agents) -> (ResourceId, Vec<StepKey>) {
  let builder = agents.profile(true);
  let reads = || agent(Some(builder), RepositoryAccess::ReadOnly);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let research = add(&mut edit, "Research", &[], reads());
  let plan = add(&mut edit, "Plan", &[&research], reads());
  let implement = add(&mut edit, "Implement", &[&plan], reads());
  let checks = add(&mut edit, "Run checks", &[&implement], reads());
  let review = add(&mut edit, "Review", &[&implement], reads());
  let notes = add(&mut edit, "Write notes", &[&checks, &review], reads());
  let id = store(agents, "Feature delivery", &edit.validate().expect("a pipeline"));
  let (waits, on) = (review.clone(), notes.clone());
  arm(id, move |edit| {
    edit.set_dependency(&waits, &on, true);
  });
  open(agents, id);
  agents.until("the notice", |s| s.exists(NOTICE));
  (id, vec![research, plan, implement, checks, review, notes])
}

/// Opens `pipeline` with `change` made to its edit, and waits for the notice.
fn opened_with(agents: &mut Agents, pipeline: ResourceId, change: impl FnOnce(&mut PipelineEdit) + Send + 'static) {
  arm(pipeline, change);
  open(agents, pipeline);
  agents.until("the notice", |s| s.exists(NOTICE));
}

#[test]
fn two_steps_that_wait_for_each_other_are_named_and_a_line_is_offered() {
  let mut agents = Agents::new(true);
  review_and_notes_wait_for_each_other(&mut agents);

  assert_eq!(
    agents.text(&format!("{NOTICE}-title")),
    "These steps have no order to draw"
  );
  assert_eq!(
    agents.text(&format!("{NOTICE}-body")),
    format!("05 Review and 06 Write notes wait for each other, so neither can start. {MAKE_A_LINE}")
  );
  assert!(agents.text(LINE).contains("Make it a line"));
  assert!(
    !agents.exists("pipeline-hub-flow-undrawable-edit"),
    "no way to the old editor"
  );
  assert!(agents.text(READY_LINE).contains("Make it a line →"));
  assert_eq!(agents.text(HINT), "Fix the order of the steps to save");
}

#[test]
fn a_longer_circle_names_every_step_in_it() {
  let mut agents = Agents::new(true);
  let (id, keys) = chain(&agents, &["A", "B", "C"]);
  let (first, last) = (keys[0].clone(), keys[2].clone());
  opened_with(&mut agents, id, move |edit| {
    edit.set_dependency(&first, &last, true);
  });

  assert_eq!(
    agents.text(&format!("{NOTICE}-body")),
    format!("01 A, 02 B and 03 C wait for each other in a circle, so none of them can start. {MAKE_A_LINE}")
  );
  assert!(agents.exists(LINE));
}

#[test]
fn a_step_that_waits_for_a_step_that_is_not_there_is_offered_a_line() {
  let mut agents = Agents::new(true);
  let (id, keys) = chain(&agents, &["A", "B"]);
  let second = keys[1].clone();
  opened_with(&mut agents, id, move |edit| {
    edit.set_dependency(&second, &StepKey::new("gone").expect("a key"), true);
  });

  assert_eq!(
    agents.text(&format!("{NOTICE}-body")),
    format!(
      "A step waits for a step that is not there, or returns on failure to a step that runs after it. {MAKE_A_LINE}"
    )
  );
  assert!(agents.exists(LINE));
  assert!(agents.exists(READY_LINE));
}

#[test]
fn two_steps_with_one_key_say_no_line_puts_them_right_and_offer_none() {
  let mut agents = Agents::new(true);
  let (id, _) = chain(&agents, &["A", "B"]);
  opened_with(&mut agents, id, |edit| {
    let mut steps = edit.steps().to_vec();
    steps.push(steps[0].clone());
    *edit = PipelineEdit::from_steps(edit.description(), edit.sources().clone(), steps);
  });

  assert_eq!(
    agents.text(&format!("{NOTICE}-body")),
    "The steps break a rule no order puts right, such as two steps with one key. Cancel puts the saved steps back."
  );
  assert!(!agents.exists(LINE), "no line to offer");
  assert!(
    agents.exists("team-hub-ready-steps"),
    "Ready? says the steps have no order"
  );
  assert!(!agents.exists(READY_LINE), "and offers no line either");
}

#[test]
fn make_it_a_line_draws_the_steps_at_once_and_save_stores_them() {
  let mut agents = Agents::new(true);
  let (id, keys) = review_and_notes_wait_for_each_other(&mut agents);

  agents.click(LINE);
  agents.until("the flow drawn", |s| {
    !s.exists(NOTICE) && s.exists(&format!("pipeline-hub-flow-step-{}", keys[5]))
  });
  assert!(
    !agents.exists("team-hub-ready-steps"),
    "Ready? no longer names the steps"
  );
  assert_ne!(agents.text(HINT), "Fix the order of the steps to save", "Save is on");

  save_until(&mut agents, "the line stored", |s| {
    let body = stored(s, id);
    body
      .steps()
      .windows(2)
      .all(|pair| pair[1].depends_on.iter().eq([&pair[0].key]))
  });
  let body = stored(&agents, id);
  assert!(body.steps()[0].depends_on.is_empty(), "the first waits for nothing");
}

#[test]
fn readys_make_it_a_line_puts_the_steps_right_too() {
  let mut agents = Agents::new(true);
  review_and_notes_wait_for_each_other(&mut agents);

  agents.click(READY_LINE);
  agents.until("the flow drawn", |s| !s.exists(NOTICE));
  assert!(!agents.exists(READY_LINE));
}

#[test]
fn the_qa_apps_marker_puts_the_same_steps_into_the_panel() {
  let mut agents = Agents::new(true);
  let (id, keys) = chain(&agents, &["A", "B"]);
  // The marker sits beside a database; the project's own folder stands in for the application data's.
  let database = agents.root.join("orchester.db");
  pipeline_edit::write_marker(&database, id, &[(keys[0].clone(), keys[1].clone())]).expect("the marker");
  assert_eq!(pipeline_edit::arm_from_marker(&database), Ok(true));
  open(&mut agents, id);
  agents.until("the notice", |s| s.exists(NOTICE));

  assert_eq!(
    agents.text(&format!("{NOTICE}-body")),
    format!("01 A and 02 B wait for each other, so neither can start. {MAKE_A_LINE}")
  );
}
