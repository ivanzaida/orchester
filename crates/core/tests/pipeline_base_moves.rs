//! Which steps of a pipeline merge the base branch when it moves (ORC-65, `Pipeline::takes_base_moves`): the step's own
//! choice, else a step with repository write that does not review or verify; never a tool step. Run with
//! `cargo test -p orchester-core --test pipeline_base_moves base_moves`.

use std::num::NonZeroU32;

use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{FailureRoute, Pipeline, PromptText, RepositoryAccess, Step, StepKey, StepKind},
};

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

fn agent(value: &str, repository: RepositoryAccess, after: &[&str]) -> Step {
  Step {
    key: key(value),
    name: Name::new(value).expect("valid name"),
    kind: StepKind::Agent {
      profile: Some(ResourceId::generate()),
      repository,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: after.iter().map(|dependency| key(dependency)).collect(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

fn returning(step: Step, to: &str) -> Step {
  Step {
    on_failure: Some(FailureRoute {
      return_to: key(to),
      max_retries: NonZeroU32::new(2).expect("positive"),
    }),
    ..step
  }
}

fn choosing(mut step: Step, takes: bool) -> Step {
  if let StepKind::Agent { permissions, .. } = &mut step.kind {
    permissions.takes_base_moves = Some(takes);
  }
  step
}

/// Implement → Check (a command tool) → Review → Verify, each check returning to Implement; Review and Verify write,
/// as a verifier that writes build output and QA files does.
fn delivery() -> Vec<Step> {
  let check = Step {
    kind: StepKind::Tool {
      tool: Some(ResourceId::generate()),
    },
    ..returning(agent("check", RepositoryAccess::ReadOnly, &["implement"]), "implement")
  };
  vec![
    agent("implement", RepositoryAccess::Write, &[]),
    check,
    returning(agent("review", RepositoryAccess::Write, &["check"]), "implement"),
    returning(agent("verify", RepositoryAccess::Write, &["review"]), "implement"),
  ]
}

fn takers(pipeline: &Pipeline) -> Vec<&str> {
  pipeline
    .steps()
    .iter()
    .filter(|step| pipeline.takes_base_moves(&step.key))
    .map(|step| step.key.as_str())
    .collect()
}

#[test]
fn base_moves_by_default_only_the_implementing_step_takes_them_never_review_or_verify() {
  let pipeline = Pipeline::new(delivery()).expect("pipeline");
  assert_eq!(takers(&pipeline), ["implement"]);
}

#[test]
fn base_moves_a_review_or_verify_key_or_a_route_back_to_a_writer_checks_work() {
  let pipeline = Pipeline::new(vec![
    agent("plan", RepositoryAccess::ReadOnly, &[]),
    // Implement returns its failures to Plan, which only reads: it still implements.
    returning(agent("implement", RepositoryAccess::Write, &["plan"]), "plan"),
    agent("document", RepositoryAccess::Write, &["implement"]),
    returning(agent("polish", RepositoryAccess::Write, &["document"]), "document"),
    agent("design-verify", RepositoryAccess::Write, &["polish"]),
    agent("code-review", RepositoryAccess::Write, &["polish"]),
  ])
  .expect("pipeline");
  assert_eq!(takers(&pipeline), ["implement", "document"]);
}

#[test]
fn base_moves_the_steps_own_choice_overrides_the_default() {
  let mut steps = delivery();
  steps[0] = choosing(steps[0].clone(), false);
  steps[3] = choosing(steps[3].clone(), true);
  let pipeline = Pipeline::new(steps).expect("pipeline");
  assert_eq!(takers(&pipeline), ["verify"]);
  assert!(
    !pipeline.takes_base_moves(&key("missing")),
    "a step that is not there takes none"
  );
}

#[test]
fn base_moves_a_pipeline_saved_without_the_choice_follows_the_default_and_a_choice_is_kept() {
  let stored = serde_json::to_value(Pipeline::new(delivery()).expect("pipeline")).expect("serialize");
  assert!(
    !stored.to_string().contains("takes_base_moves"),
    "no choice, nothing stored: {stored}"
  );
  let read: Pipeline = serde_json::from_value(stored).expect("an older pipeline reads");
  assert_eq!(takers(&read), ["implement"]);

  let mut steps = delivery();
  steps[3] = choosing(steps[3].clone(), true);
  let chosen = serde_json::to_value(Pipeline::new(steps).expect("pipeline")).expect("serialize");
  let read: Pipeline = serde_json::from_value(chosen).expect("reads");
  assert_eq!(takers(&read), ["implement", "verify"]);
}
