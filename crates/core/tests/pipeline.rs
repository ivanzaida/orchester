use std::{collections::BTreeSet, num::NonZeroU32};

use orchester_core::{
  id::ResourceId,
  money::{Currency, Money},
  name::Name,
  resource::{FailureRoute, Pipeline, PipelineError, PromptText, RepositoryAccess, Step, StepKey, StepKind},
};

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

fn step(value: &str, depends_on: &[&str]) -> Step {
  Step {
    key: key(value),
    name: Name::new(value).expect("valid name"),
    kind: StepKind::Agent {
      profile: ResourceId::generate(),
      repository: RepositoryAccess::ReadOnly,
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
      profile: ResourceId::generate(),
      repository: RepositoryAccess::Write,
    },
    instructions: PromptText::new("Implement {{task.title}}.").expect("valid prompt"),
    budget: Some(Money::from_micros(2_000_000, Currency::USD)),
    ..step("implement", &["plan"])
  };
  let checks = Step {
    kind: StepKind::Tool {
      tool: ResourceId::generate(),
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
fn pipeline_needs_steps() {
  assert_eq!(Pipeline::new(Vec::new()), Err(PipelineError::NoSteps));
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
fn pipeline_rejects_non_positive_step_budgets() {
  let mut steps = feature_delivery();
  steps[0].budget = Some(Money::from_micros(0, Currency::USD));
  assert_eq!(
    Pipeline::new(steps),
    Err(PipelineError::NonPositiveBudget(key("research")))
  );
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
