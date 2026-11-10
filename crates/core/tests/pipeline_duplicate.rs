//! Duplicating a step in place (PV2-17 items 19 and 4 of round 2, D-023): the copy runs right after the step, in
//! series, also when the step is in a parallel group and when the steps have no flow to place it in.
//! `cargo test -p orchester-core --test pipeline_duplicate`.

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    PipelineEdit, PipelineSources, PromptText, StarterAssignments, StarterStep, StarterStepText, Step, StepKey,
    StepKind, feature_delivery,
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

/// a → b → c → d
fn linear() -> PipelineEdit {
  PipelineEdit::from_steps(
    "",
    PipelineSources::default(),
    vec![step("a", &[]), step("b", &["a"]), step("c", &["b"]), step("d", &["c"])],
  )
}

/// What step `name` waits for.
fn waits(edit: &PipelineEdit, name: &str) -> Vec<String> {
  edit
    .step(&key(name))
    .unwrap_or_else(|| panic!("{name} is a step"))
    .depends_on
    .iter()
    .map(ToString::to_string)
    .collect()
}

fn text(starter: StarterStep) -> StarterStepText {
  StarterStepText {
    name: Name::new(starter.key()).expect("valid name"),
    instructions: PromptText::new("Work on {{task.title}}.").expect("valid prompt"),
  }
}

/// 01 research → 02 plan → 03 implement → { 04 run-checks ∥ 05 review } → 06 create-pull-request.
fn feature() -> PipelineEdit {
  let some = || Some(ResourceId::generate());
  PipelineEdit::new(feature_delivery(
    text,
    StarterAssignments {
      explorer: some(),
      planner: some(),
      builder: some(),
      reviewer: some(),
      checks: some(),
      pull_request: some(),
    },
  ))
}

#[test]
fn a_copy_runs_right_after_its_step() {
  let mut edit = linear();
  assert_eq!(edit.duplicate_after(&key("b")), Some(key("b-copy")));
  assert_eq!(waits(&edit, "b-copy"), ["b"]);
  assert_eq!(
    waits(&edit, "c"),
    ["b-copy"],
    "what waited for the step waits for the copy"
  );
  assert_eq!(waits(&edit, "b"), ["a"], "the step stays where it was");
  edit.validate().expect("still a pipeline");
}

#[test]
fn a_copy_of_a_step_in_a_group_runs_after_the_whole_group() {
  let mut edit = feature();
  let copy = edit.duplicate_after(&key("run-checks")).expect("a copy");
  assert_eq!(waits(&edit, copy.as_str()), ["review", "run-checks"]);
  assert_eq!(waits(&edit, "create-pull-request"), [copy.to_string()]);
  assert_eq!(waits(&edit, "review"), ["implement"], "the group is left as it was");
  edit.validate().expect("still a pipeline");
}

#[test]
fn a_copy_runs_after_its_step_when_the_steps_have_no_flow() {
  let mut edit = linear();
  // a waits for d: a cycle, so there is no flow to place the copy in.
  assert!(edit.set_dependency(&key("a"), &key("d"), true));
  assert_eq!(edit.duplicate_after(&key("b")), Some(key("b-copy")));
  assert_eq!(waits(&edit, "b-copy"), ["b"]);
  assert_eq!(waits(&edit, "c"), ["b-copy"]);
}

#[test]
fn there_is_no_copy_of_a_step_the_pipeline_has_not() {
  let mut edit = linear();
  assert_eq!(edit.duplicate_after(&key("zzz")), None);
  assert_eq!(edit.steps().len(), 4);
}
