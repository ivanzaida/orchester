use std::{collections::BTreeSet, num::NonZeroU32};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    FailureRoute, ModelId, Pipeline, PipelineError, PromptText, PublishError, RepositoryAccess, ResourceBody, Step,
    StepKey, StepKind,
  },
  tokens::TokenBudget,
};

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

fn step(value: &str, depends_on: &[&str]) -> Step {
  Step {
    key: key(value),
    name: Name::new(value).expect("valid name"),
    kind: StepKind::Agent {
      profile: Some(ResourceId::generate()),
      repository: RepositoryAccess::ReadOnly,
      model: None,
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: depends_on.iter().map(|dependency| key(dependency)).collect(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

fn retry(target: &str, max_retries: u32) -> Option<FailureRoute> {
  Some(FailureRoute {
    return_to: key(target),
    max_retries: NonZeroU32::new(max_retries).expect("positive retries"),
  })
}

/// Research → Plan (approval) → Implement → Run checks ∥ Review → Create pull request.
fn feature_delivery() -> Vec<Step> {
  let plan = Step {
    approval_gate: true,
    ..step("plan", &["research"])
  };
  let implement = Step {
    kind: StepKind::Agent {
      profile: Some(ResourceId::generate()),
      repository: RepositoryAccess::Write,
      model: Some(ModelId::new("opus").expect("valid model")),
    },
    instructions: PromptText::new("Implement {{task.title}}.").expect("valid prompt"),
    budget: Some(TokenBudget::new(400_000).expect("above zero")),
    ..step("implement", &["plan"])
  };
  let checks = Step {
    kind: StepKind::Tool {
      tool: Some(ResourceId::generate()),
    },
    on_failure: retry("implement", 3),
    ..step("run-checks", &["implement"])
  };
  let review = Step {
    on_failure: retry("implement", 2),
    ..step("review", &["implement"])
  };
  vec![
    step("research", &[]),
    plan,
    implement,
    checks,
    review,
    step("create-pull-request", &["run-checks", "review"]),
  ]
}

#[test]
fn pipeline_accepts_parallel_steps_gates_and_upstream_retries() {
  let pipeline = Pipeline::new(feature_delivery()).unwrap();
  assert_eq!(pipeline.steps().len(), 6);
  assert!(pipeline.step(&key("plan")).unwrap().approval_gate);
  let join = pipeline.step(&key("create-pull-request")).unwrap();
  assert_eq!(join.depends_on, BTreeSet::from([key("review"), key("run-checks")]));
  assert_eq!(pipeline.steps()[0].key, key("research"));
}

#[test]
fn pipeline_stages_group_steps_that_can_run_beside_each_other() {
  let pipeline = Pipeline::new(feature_delivery()).expect("valid pipeline");
  let stages: Vec<Vec<&str>> = pipeline
    .stages()
    .iter()
    .map(|stage| stage.iter().map(|step| step.key.as_str()).collect())
    .collect();
  assert_eq!(
    stages,
    [
      vec!["research"],
      vec!["plan"],
      vec!["implement"],
      vec!["run-checks", "review"],
      vec!["create-pull-request"],
    ]
  );
  assert!(Pipeline::new(Vec::new()).expect("empty").stages().is_empty());
}

#[test]
fn pipeline_without_steps_is_a_draft_that_cannot_be_published() {
  let empty = Pipeline::new(Vec::new()).expect("an empty draft is valid");
  assert_eq!(empty.check_publishable(), Err(PublishError::NoSteps));
}

#[test]
fn pipeline_description_survives_json() {
  let pipeline = Pipeline::new(vec![step("build", &[])])
    .expect("valid")
    .with_description("From a brief to a pull request.".to_owned());
  let json = serde_json::to_string(&pipeline).expect("serialize");
  let read: Pipeline = serde_json::from_str(&json).expect("deserialize");
  assert_eq!(read.description(), "From a brief to a pull request.");
  let without: Pipeline = serde_json::from_str(r#"{"steps":[]}"#).expect("description is optional");
  assert_eq!(without.description(), "");
}

#[test]
fn pipeline_rejects_duplicate_step_keys() {
  let steps = vec![step("build", &[]), step("build", &[])];
  assert_eq!(Pipeline::new(steps), Err(PipelineError::DuplicateStep(key("build"))));
}

#[test]
fn pipeline_rejects_unknown_dependencies() {
  let steps = vec![step("build", &["setup"])];
  assert_eq!(
    Pipeline::new(steps),
    Err(PipelineError::UnknownDependency {
      step: key("build"),
      dependency: key("setup"),
    })
  );
}

#[test]
fn pipeline_rejects_cycles() {
  let steps = vec![step("a", &["c"]), step("b", &["a"]), step("c", &["b"])];
  assert!(matches!(Pipeline::new(steps), Err(PipelineError::Cycle(_))));
  let self_dependent = vec![step("a", &["a"])];
  assert_eq!(Pipeline::new(self_dependent), Err(PipelineError::Cycle(key("a"))));
}

#[test]
fn pipeline_allows_dependencies_on_later_listed_steps() {
  let steps = vec![step("review", &["build"]), step("build", &[])];
  assert!(Pipeline::new(steps).is_ok());
}

#[test]
fn pipeline_rejects_unknown_retry_targets() {
  let mut steps = feature_delivery();
  steps[3].on_failure = retry("implementation", 3);
  assert_eq!(
    Pipeline::new(steps),
    Err(PipelineError::UnknownRetryTarget {
      step: key("run-checks"),
      target: key("implementation"),
    })
  );
}

#[test]
fn pipeline_rejects_retry_targets_that_do_not_precede_the_step() {
  let cases = [
    ("run-checks", "run-checks"),
    ("run-checks", "review"),
    ("implement", "create-pull-request"),
  ];
  for (failing, target) in cases {
    let mut steps = feature_delivery();
    let position = steps.iter().position(|step| step.key == key(failing)).unwrap();
    steps[position].on_failure = retry(target, 1);
    assert_eq!(
      Pipeline::new(steps),
      Err(PipelineError::RetryTargetNotUpstream {
        step: key(failing),
        target: key(target),
      }),
      "{failing} → {target}"
    );
  }
}

#[test]
fn pipeline_step_budgets_are_tokens_and_money_budgets_read_as_none() {
  let pipeline = Pipeline::new(feature_delivery()).expect("valid pipeline");
  let json = serde_json::to_value(&pipeline).expect("serializable");
  assert_eq!(json["steps"][2]["budget"], serde_json::json!(400_000));
  let mut legacy = json.clone();
  legacy["steps"][2]["budget"] = serde_json::json!({ "micros": 2_000_000, "currency": "USD" });
  let read: Pipeline = serde_json::from_value(legacy).expect("a published money budget still reads");
  assert_eq!(read.step(&key("implement")).and_then(|step| step.budget), None);
  let mut without = json;
  without["steps"][2]
    .as_object_mut()
    .expect("a step is an object")
    .remove("budget");
  let read: Pipeline = serde_json::from_value(without).expect("a step without a budget reads");
  assert_eq!(read.step(&key("implement")).and_then(|step| step.budget), None);
}

#[test]
fn pipeline_step_keys_are_slugs() {
  assert!(StepKey::new("run-checks").is_ok());
  assert!(StepKey::new("Run checks").is_err());
}

#[test]
fn pipeline_body_round_trips_and_revalidates_on_read() {
  let pipeline = Pipeline::new(feature_delivery()).unwrap();
  let json = serde_json::to_string(&pipeline).unwrap();
  assert_eq!(serde_json::from_str::<Pipeline>(&json).unwrap(), pipeline);

  let cyclic = json.replacen(r#""depends_on":[]"#, r#""depends_on":["create-pull-request"]"#, 1);
  assert_ne!(cyclic, json);
  let error = serde_json::from_str::<Pipeline>(&cyclic).unwrap_err();
  assert!(error.to_string().contains("depends on itself"), "{error}");
}
