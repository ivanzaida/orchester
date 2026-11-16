//! A pipeline without a Deliver step in the Pipelines hub (`Pipelines · v2` P15b `uDfc0`; PV2-23): *Add Deliver step*
//! stands where the Deliver card would, adds the step with the defaults and opens it in the panel; *Cancel* drops it
//! again; a route to a Deliver step the pipeline has not shows the pipeline; the strip and its keys reach the Deliver
//! step; while the question about a save waits, neither *Add Deliver step* nor the Deliver step's fields change
//! anything. `cargo test -p orchester-app --test agents pipeline_deliver_add`.

use orchester_app::routes::PipelinesFilter;
use orchester_core::resource::{DeliverStep, RepositoryAccess, StepKey};

use super::{
  harness::Agents,
  pipeline_deliver::{
    ADD, AUTO, HEAD, Made, RECHECK_HINT, deliver_route, open_deliver, pipeline, pipeline_route, unsaved,
  },
  pipeline_hub::open,
  pipeline_hub_menu::key,
  pipeline_step::{CANCEL, SAVE, keys, named, open_step, route, save_until, stored},
  pipeline_step_gate::claude,
};

const CONFIRM: &str = "pipeline-hub-confirm-confirm";

#[test]
fn pipeline_deliver_add_stands_where_deliver_would_and_adds_the_default_step_until_save() {
  let mut agents = Agents::new(true);
  let builder = claude(&agents, "Builder", RepositoryAccess::Write, &["cargo test"]);
  let id = pipeline(
    &agents,
    "No Deliver",
    &Made {
      writers: &[builder],
      ..Made::default()
    },
  );
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.until("Add Deliver step", |s| s.exists(ADD));
  assert_eq!(agents.text(&format!("{ADD}-name")), "Add Deliver step");
  assert_eq!(
    agents.text(&format!("{ADD}-line")),
    "Runs end in Ready for review and you merge them from the task."
  );
  assert!(
    agents.exists("pipeline-hub-flow-row-add-deliver-node"),
    "a node of its own on the line"
  );

  agents.click(ADD);
  agents.until("the Deliver step", |s| s.exists(HEAD));
  assert_eq!(route(&agents), deliver_route(id));
  assert!(agents.exists("pipeline-hub-deliver-merge-commit"));
  assert!(unsaved(&mut agents, id));
  assert!(stored(&agents, id).deliver().is_none(), "nothing stored before Save");

  save_until(&mut agents, "the Deliver step stored", |s| {
    stored(s, id).deliver() == Some(&DeliverStep::default())
  });
}

#[test]
fn pipeline_deliver_add_then_cancel_shows_the_pipeline_without_it() {
  let mut agents = Agents::new(true);
  let builder = claude(&agents, "Builder", RepositoryAccess::Write, &["cargo test"]);
  let id = pipeline(
    &agents,
    "No Deliver",
    &Made {
      writers: &[builder],
      ..Made::default()
    },
  );
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.until("Add Deliver step", |s| s.exists(ADD));
  agents.click(ADD);
  agents.until("the Deliver step", |s| s.exists(HEAD));

  agents.click(CANCEL);
  agents.until("the pipeline", |s| s.exists(ADD));
  assert_eq!(
    route(&agents),
    pipeline_route(id),
    "the route no longer names a Deliver step the pipeline has not"
  );
  assert!(!agents.exists(HEAD));
}

#[test]
fn pipeline_deliver_a_route_to_a_deliver_step_the_pipeline_has_not_shows_the_pipeline() {
  let mut agents = Agents::new(true);
  let builder = claude(&agents, "Builder", RepositoryAccess::Write, &["cargo test"]);
  let id = pipeline(
    &agents,
    "No Deliver",
    &Made {
      writers: &[builder],
      ..Made::default()
    },
  );
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.window.navigation().go(deliver_route(id));
  agents.until("the route corrected", |s| route(s) == pipeline_route(id));
  agents.until("the pipeline", |s| s.exists(ADD));
  assert!(!agents.exists("screen-pipelines"), "never the old Deliver page");
}

#[test]
fn pipeline_deliver_the_strip_s_keys_walk_to_the_deliver_step_and_back_keeping_the_edit() {
  let mut agents = Agents::new(true);
  let builder = claude(&agents, "Builder", RepositoryAccess::Write, &["cargo test"]);
  let id = pipeline(
    &agents,
    "Feature delivery",
    &Made {
      writers: &[builder],
      deliver: Some(DeliverStep::default()),
      ..Made::default()
    },
  );
  let [only]: [StepKey; 1] = keys(&agents, id).try_into().expect("one step");
  open_step(&mut agents, id, &only);
  agents.set(super::pipeline_step::INSTRUCTIONS, "Typed before Deliver");
  agents.until("held", |s| unsaved(s, id));

  key(&mut agents, "ArrowRight");
  agents.until("the Deliver step", |s| s.exists(HEAD));
  assert_eq!(route(&agents), deliver_route(id));
  assert!(
    !agents.exists("pipeline-hub-leave"),
    "no question: the edit stays where it is"
  );
  assert!(unsaved(&mut agents, id));

  key(&mut agents, "ArrowLeft");
  agents.until("the step before", |s| named(s, "Step 1"));
  key(&mut agents, "ArrowRight");
  agents.until("Deliver again", |s| s.exists(HEAD));
  key(&mut agents, "ArrowRight");
  agents.until("a pass", |_| true);
  assert_eq!(route(&agents), deliver_route(id), "nothing runs after Deliver");
}

#[test]
fn pipeline_deliver_add_is_faded_and_takes_no_press_while_the_question_waits() {
  let mut agents = Agents::new(true);
  let builder = claude(&agents, "Builder", RepositoryAccess::Write, &["cargo test"]);
  let id = pipeline(
    &agents,
    "Gated",
    &Made {
      writers: &[builder],
      gated: true,
      ..Made::default()
    },
  );
  ask_before_saving(&mut agents, id);
  agents.click("pipeline-hub-step-back");
  agents.until("the pipeline", |s| s.exists(ADD));
  agents.click(ADD);
  agents.until("a pass", |_| true);
  assert_eq!(route(&agents), pipeline_route(id), "Add Deliver step does nothing");
  assert!(agents.exists(ADD));
}

#[test]
fn pipeline_deliver_fields_take_no_press_while_the_question_waits() {
  let mut agents = Agents::new(true);
  let builder = claude(&agents, "Builder", RepositoryAccess::Write, &["cargo test"]);
  let id = pipeline(
    &agents,
    "Gated",
    &Made {
      writers: &[builder],
      gated: true,
      deliver: Some(DeliverStep::default()),
      ..Made::default()
    },
  );
  open_deliver(&mut agents, id);
  let before = agents.text(RECHECK_HINT);
  ask_before_saving(&mut agents, id);
  agents.click("pipeline-hub-step-chip-deliver");
  agents.until("the Deliver step", |s| s.exists(HEAD));

  agents.click(AUTO);
  agents.click("pipeline-hub-deliver-squash");
  agents.click("pipeline-hub-deliver-remove");
  agents.until("a pass", |_| true);
  assert_eq!(agents.text(RECHECK_HINT), before, "the mode stays");
  assert!(!agents.exists("pipeline-hub-deliver-squash-body"), "the method stays");
  assert_eq!(route(&agents), deliver_route(id), "Remove step does nothing");
}

/// Opens the first step of `id`, turns its approval off and presses *Save*, which asks before a save that lets the run
/// go on without the person; the question stays open.
fn ask_before_saving(agents: &mut Agents, id: orchester_core::id::ResourceId) {
  let first = keys(agents, id)[0].clone();
  open_step(agents, id, &first);
  agents.click("pipeline-hub-step-approval");
  agents.press_until(SAVE, "the question", |s| s.exists(CONFIRM));
}
