//! The flow a pipeline's steps are drawn as, stage by stage, and the steps that have no order to draw.

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    FlowError, PipelineError, PromptText, StarterAssignments, StarterStep, StarterStepText, Step, StepKey, StepKind,
    feature_delivery, flow_units,
  },
};

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

fn step(name: &str, depends_on: &[&str]) -> Step {
  Step {
    key: key(name),
    name: Name::new(name).expect("valid name"),
    kind: StepKind::Tool { tool: None },
    instructions: PromptText::new("Work on {{task.title}}.").expect("valid prompt"),
    depends_on: depends_on.iter().map(|dependency| key(dependency)).collect(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

fn drawn(steps: &[Step]) -> Result<Vec<Vec<&str>>, FlowError> {
  let units = flow_units(steps)?;
  Ok(
    units
      .iter()
      .map(|unit| {
        unit
          .iter()
          .map(|key| {
            steps
              .iter()
              .find(|step| &step.key == key)
              .expect("a step of the steps")
              .key
              .as_str()
          })
          .collect()
      })
      .collect(),
  )
}

#[test]
fn linear_steps_are_a_unit_each() {
  let steps = [step("a", &[]), step("b", &["a"]), step("c", &["b"])];
  assert_eq!(drawn(&steps), Ok(vec![vec!["a"], vec!["b"], vec!["c"]]));
}

#[test]
fn steps_that_wait_for_the_same_steps_are_one_parallel_unit() {
  let steps = [
    step("a", &[]),
    step("b", &["a"]),
    step("c", &["a"]),
    step("d", &["b", "c"]),
  ];
  assert_eq!(drawn(&steps), Ok(vec![vec!["a"], vec!["b", "c"], vec!["d"]]));
}

#[test]
fn more_than_two_parallel_steps_stay_in_one_unit() {
  let steps = [
    step("a", &[]),
    step("b", &["a"]),
    step("c", &["a"]),
    step("d", &["a"]),
    step("e", &["b", "c", "d"]),
  ];
  assert_eq!(drawn(&steps), Ok(vec![vec!["a"], vec!["b", "c", "d"], vec!["e"]]));
}

#[test]
fn steps_without_dependencies_run_side_by_side_at_the_start() {
  let steps = [step("a", &[]), step("b", &[]), step("c", &["a", "b"])];
  assert_eq!(drawn(&steps), Ok(vec![vec!["a", "b"], vec!["c"]]));
}

#[test]
fn a_dependency_another_one_already_implies_is_not_a_branch() {
  let steps = [step("a", &[]), step("b", &["a"]), step("c", &["a", "b"])];
  assert_eq!(drawn(&steps), Ok(vec![vec!["a"], vec!["b"], vec!["c"]]));
}

#[test]
fn independent_chains_are_drawn_by_stage() {
  let steps = [step("a", &[]), step("b", &["a"]), step("c", &[]), step("d", &["c"])];
  assert_eq!(drawn(&steps), Ok(vec![vec!["a", "c"], vec!["b", "d"]]));
}

#[test]
fn a_step_that_waits_for_only_some_of_a_group_stands_under_the_group() {
  let steps = [step("a", &[]), step("b", &["a"]), step("c", &["a"]), step("d", &["b"])];
  assert_eq!(drawn(&steps), Ok(vec![vec!["a"], vec!["b", "c"], vec!["d"]]));
}

#[test]
fn steps_that_wait_for_each_other_in_a_circle_have_no_flow() {
  let steps = [step("a", &["b"]), step("b", &["a"])];
  assert!(matches!(
    flow_units(&steps),
    Err(FlowError::Broken(PipelineError::Cycle(_)))
  ));
}

#[test]
fn a_dependency_on_a_missing_step_has_no_flow() {
  let steps = [step("a", &["gone"])];
  assert!(matches!(
    flow_units(&steps),
    Err(FlowError::Broken(PipelineError::UnknownDependency { .. }))
  ));
}

#[test]
fn no_steps_are_no_units() {
  assert_eq!(flow_units(&[]), Ok(Vec::new()));
}

fn text(starter: StarterStep) -> StarterStepText {
  StarterStepText {
    name: Name::new(starter.key()).expect("valid name"),
    instructions: PromptText::new("Work on {{task.title}}.").expect("valid prompt"),
  }
}

#[test]
fn the_starter_pipeline_is_drawn_with_its_checks_and_review_side_by_side() {
  let assignments = StarterAssignments {
    explorer: Some(ResourceId::generate()),
    planner: Some(ResourceId::generate()),
    builder: Some(ResourceId::generate()),
    reviewer: Some(ResourceId::generate()),
    checks: Some(ResourceId::generate()),
    pull_request: Some(ResourceId::generate()),
  };
  let pipeline = feature_delivery(text, assignments);
  assert_eq!(
    drawn(pipeline.steps()),
    Ok(vec![
      vec!["research"],
      vec!["plan"],
      vec!["implement"],
      vec!["run-checks", "review"],
      vec!["create-pull-request"],
    ])
  );
}
