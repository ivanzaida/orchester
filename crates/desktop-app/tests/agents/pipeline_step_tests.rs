//! A Test step made in the Pipelines hub: the Agent picker lists the project's test suites after its tools, and
//! choosing them makes the step a Test step, saved with the pipeline.
//! `cargo test -p orchester-app --test agents pipeline_step_tests`.

use orchester_core::resource::StepKind;

use super::{
  pipeline_hub::setup,
  pipeline_step::{keys, open_step, save_until},
  pipeline_step_agent::{FIELD, PICKER, checks, executor, item},
};

const TESTS: &str = "Test suites of the changed areas";

#[test]
fn the_picker_offers_the_test_suites_after_the_tools_and_choosing_them_makes_a_test_step() {
  let (mut agents, [ready, ..]) = setup();
  checks(&agents);
  let plan = keys(&agents, ready)[0].clone();
  open_step(&mut agents, ready, &plan);
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(2)));
  assert_eq!(agents.text(&format!("{}-name", item(1))), "Checks");
  assert_eq!(agents.text(&format!("{}-name", item(2))), TESTS);
  assert_eq!(agents.text(&format!("{}-runs", item(2))), "Failed tests fail the step");

  agents.click(&item(2));
  agents.until("the suites run the step", |s| {
    s.text("pipeline-hub-step-agent-name") == TESTS && !s.exists(PICKER)
  });
  assert!(
    matches!(executor(&agents, ready, 0), StepKind::Agent { .. }),
    "nothing is stored before Save"
  );
  save_until(&mut agents, "saved", |s| executor(s, ready, 0) == StepKind::Test);
}

#[test]
fn a_project_with_no_tools_still_offers_the_test_suites() {
  let (mut agents, [ready, ..]) = setup();
  let plan = keys(&agents, ready)[0].clone();
  open_step(&mut agents, ready, &plan);
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(1)));
  assert_eq!(agents.text(&format!("{}-name", item(1))), TESTS);
  assert_eq!(agents.text(&format!("{PICKER}-group-tools")), "Tools");
}
