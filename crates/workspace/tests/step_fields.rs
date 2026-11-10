//! What choosing another agent for a step does to its model override (PV2-17 item 24): it stays while the step runs on
//! the CLI it was set for, and is dropped, and handed back to be said, when the step moves to another CLI or to a tool.
//! `cargo test -p orchester-workspace --test step_fields`.

use orchester_core::{
  connection::AgentProvider,
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{ModelId, Pipeline, PipelineEdit, RepositoryAccess, Step, StepKind},
};
use orchester_workspace::pipelines::fields::{Executor, ModelChoice, dropped_override, take_back_override};

/// A step that runs a profile and overrides its model with `custom-model`.
fn overriding() -> Step {
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let key = edit.add_step(Name::new("Plan").expect("name"), None);
  edit.update(&key, |step| {
    step.kind = StepKind::Agent {
      profile: Some(ResourceId::generate()),
      repository: RepositoryAccess::Write,
      model: Some(ModelId::new("custom-model").expect("model")),
      permissions: StepPermissions::default(),
    };
  });
  edit.step(&key).expect("the step").clone()
}

fn model_of(step: &Step) -> Option<String> {
  ModelChoice::of(step).0.map(|model| model.as_str().to_owned())
}

#[test]
fn another_profile_on_the_same_cli_keeps_the_override() {
  let mut step = overriding();
  let other = ResourceId::generate();
  let dropped = Executor::Profile(other).apply_on(
    &mut step,
    (Some(AgentProvider::CodexCli), Some(AgentProvider::CodexCli)),
  );
  assert_eq!(dropped, None);
  assert_eq!(model_of(&step).as_deref(), Some("custom-model"));
  assert_eq!(Executor::of(&step), Executor::Profile(other));
  assert!(matches!(
    step.kind,
    StepKind::Agent {
      repository: RepositoryAccess::Write,
      ..
    }
  ));
}

#[test]
fn a_profile_on_another_cli_drops_the_override_and_says_which() {
  let mut step = overriding();
  let other = ResourceId::generate();
  let dropped = Executor::Profile(other).apply_on(
    &mut step,
    (Some(AgentProvider::CodexCli), Some(AgentProvider::ClaudeCodeCli)),
  );
  assert_eq!(
    dropped.map(|model| model.as_str().to_owned()).as_deref(),
    Some("custom-model")
  );
  assert_eq!(model_of(&step), None, "the step runs on the new profile's own model");
  assert_eq!(Executor::of(&step), Executor::Profile(other));
}

#[test]
fn a_cli_that_is_not_known_keeps_the_override() {
  for clis in [
    (None, Some(AgentProvider::ClaudeCodeCli)),
    (Some(AgentProvider::CodexCli), None),
    (None, None),
  ] {
    let mut step = overriding();
    assert_eq!(
      Executor::Profile(ResourceId::generate()).apply_on(&mut step, clis),
      None
    );
    assert_eq!(model_of(&step).as_deref(), Some("custom-model"), "{clis:?}");
  }
}

#[test]
fn a_tool_drops_the_override() {
  let mut step = overriding();
  let tool = ResourceId::generate();
  let dropped = Executor::Tool(tool).apply_on(&mut step, (Some(AgentProvider::CodexCli), None));
  assert_eq!(
    dropped.map(|model| model.as_str().to_owned()).as_deref(),
    Some("custom-model")
  );
  assert_eq!(Executor::of(&step), Executor::Tool(tool));
}

#[test]
fn a_step_without_an_override_drops_nothing() {
  let mut step = overriding();
  ModelChoice(None).apply(&mut step);
  let dropped = Executor::Profile(ResourceId::generate()).apply_on(
    &mut step,
    (Some(AgentProvider::CodexCli), Some(AgentProvider::ClaudeCodeCli)),
  );
  assert_eq!(dropped, None);
}

#[test]
fn what_was_dropped_is_read_from_the_step_as_it_started() {
  let started = overriding();
  let mut step = started.clone();
  assert_eq!(
    dropped_override(&started, &step),
    None,
    "nothing chosen, nothing dropped"
  );

  Executor::Profile(ResourceId::generate()).apply_on(
    &mut step,
    (Some(AgentProvider::CodexCli), Some(AgentProvider::ClaudeCodeCli)),
  );
  assert_eq!(
    dropped_override(&started, &step)
      .map(|model| model.as_str().to_owned())
      .as_deref(),
    Some("custom-model")
  );

  let mut tool = started.clone();
  Executor::Tool(ResourceId::generate()).apply_on(&mut tool, (None, None));
  assert!(dropped_override(&started, &tool).is_some(), "a tool drops it too");

  let mut kept = started.clone();
  Executor::Profile(ResourceId::generate()).apply_on(&mut kept, (None, Some(AgentProvider::CodexCli)));
  assert_eq!(
    dropped_override(&started, &kept),
    None,
    "an override kept is not dropped"
  );
}

#[test]
fn choosing_the_overrides_cli_again_takes_it_back() {
  let started = overriding();
  let mut step = started.clone();
  let (codex, claude) = (Some(AgentProvider::CodexCli), Some(AgentProvider::ClaudeCodeCli));
  Executor::Profile(ResourceId::generate()).apply_on(&mut step, (codex, claude));
  assert!(
    !take_back_override(&mut step, &started, (codex, claude)),
    "not on another CLI"
  );
  assert_eq!(model_of(&step), None);

  Executor::Profile(ResourceId::generate()).apply_on(&mut step, (claude, codex));
  assert!(take_back_override(&mut step, &started, (codex, codex)));
  assert_eq!(model_of(&step).as_deref(), Some("custom-model"));
  assert_eq!(dropped_override(&started, &step), None);

  let mut unknown = started.clone();
  Executor::Tool(ResourceId::generate()).apply_on(&mut unknown, (None, None));
  Executor::Profile(ResourceId::generate()).apply(&mut unknown);
  assert!(
    !take_back_override(&mut unknown, &started, (None, None)),
    "a CLI that is not known takes nothing back on a guess"
  );
}
