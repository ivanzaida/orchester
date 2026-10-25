use std::num::NonZeroU32;

use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{
    EditProblem, FailureRoute, Pipeline, PipelineChange, PipelineEdit, PipelineError, PromptText, PublishError,
    RepositoryAccess, ResourceBody, StarterAssignments, StarterStep, StarterStepText, StepChange, StepKey, StepKind,
    changes, feature_delivery,
  },
  tokens::TokenBudget,
};

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn keys<'a>(keys: impl IntoIterator<Item = &'a StepKey>) -> Vec<&'a str> {
  keys.into_iter().map(StepKey::as_str).collect()
}

fn text(step: StarterStep) -> StarterStepText {
  StarterStepText {
    name: name(step.key()),
    instructions: PromptText::new("Work on {{task.title}}.").expect("valid prompt"),
  }
}

fn assigned() -> StarterAssignments {
  StarterAssignments {
    explorer: Some(ResourceId::generate()),
    planner: Some(ResourceId::generate()),
    builder: Some(ResourceId::generate()),
    reviewer: Some(ResourceId::generate()),
    checks: Some(ResourceId::generate()),
    pull_request: Some(ResourceId::generate()),
  }
}

fn starter_edit() -> PipelineEdit {
  PipelineEdit::new(feature_delivery(text, assigned()))
}

fn depends_on(edit: &PipelineEdit, step: &str) -> Vec<String> {
  let step = edit.step(&key(step)).expect("step exists");
  step.depends_on.iter().map(|key| key.as_str().to_owned()).collect()
}

#[test]
fn starter_has_the_design_steps_in_order() {
  let pipeline = feature_delivery(text, assigned());
  assert_eq!(
    keys(pipeline.steps().iter().map(|step| &step.key)),
    [
      "research",
      "plan",
      "implement",
      "run-checks",
      "review",
      "create-pull-request"
    ]
  );
  let edit = PipelineEdit::new(pipeline.clone());
  assert_eq!(depends_on(&edit, "run-checks"), ["implement"]);
  assert_eq!(depends_on(&edit, "review"), ["implement"]);
  assert_eq!(depends_on(&edit, "create-pull-request"), ["review", "run-checks"]);
  let gated: Vec<&str> = keys(
    pipeline
      .steps()
      .iter()
      .filter(|step| step.approval_gate)
      .map(|step| &step.key),
  );
  assert_eq!(gated, ["plan"]);
  for retrying in ["run-checks", "review"] {
    let route = pipeline.step(&key(retrying)).and_then(|step| step.on_failure.clone());
    let route = route.expect("verification returns to implement");
    assert_eq!(route.return_to, key("implement"));
    assert_eq!(route.max_retries.get(), 2);
  }
}

#[test]
fn starter_uses_the_given_profiles_and_tools() {
  let assignments = assigned();
  let pipeline = feature_delivery(text, assignments);
  let implement = pipeline.step(&key("implement")).expect("implement");
  assert_eq!(
    implement.kind,
    StepKind::Agent {
      profile: assignments.builder,
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    }
  );
  let checks = pipeline.step(&key("run-checks")).expect("run checks");
  assert_eq!(
    checks.kind,
    StepKind::Tool {
      tool: assignments.checks
    }
  );
  assert_eq!(pipeline.unassigned_steps().count(), 0);
  assert_eq!(pipeline.check_publishable(), Ok(()));
}

#[test]
fn starter_without_profiles_is_a_draft_that_cannot_be_published() {
  let pipeline = feature_delivery(text, StarterAssignments::default());
  assert_eq!(keys(pipeline.unassigned_steps()).len(), 6);
  let partly = feature_delivery(
    text,
    StarterAssignments {
      explorer: Some(ResourceId::generate()),
      ..StarterAssignments::default()
    },
  );
  assert_eq!(
    partly.check_publishable(),
    Err(PublishError::UnassignedSteps(
      ["plan", "implement", "run-checks", "review", "create-pull-request"]
        .map(key)
        .to_vec()
    ))
  );
}

#[test]
fn added_step_runs_after_the_selected_one_without_a_profile() {
  let mut edit = starter_edit();
  let added = edit.add_step(name("Security audit"), Some(&key("review")));
  assert_eq!(added.as_str(), "step");
  let position = edit.steps().iter().position(|step| step.key == added);
  assert_eq!(position, Some(5));
  assert_eq!(depends_on(&edit, "step"), ["review"]);
  assert_eq!(keys(edit.unassigned_steps()), ["step"]);
  assert!(edit.validate().is_ok());

  let second = edit.add_step(name("Another"), None);
  assert_eq!(second.as_str(), "step-2");
  assert_eq!(edit.steps().last().map(|step| &step.key), Some(&second));
  assert!(depends_on(&edit, "step-2").is_empty());
}

#[test]
fn duplicated_step_runs_in_parallel_with_the_original() {
  let mut edit = starter_edit();
  let copy = edit.duplicate_step(&key("review")).expect("review exists");
  assert_eq!(copy.as_str(), "review-copy");
  assert_eq!(depends_on(&edit, "review-copy"), ["implement"]);
  let original = edit.step(&key("review")).expect("review").clone();
  let duplicate = edit.step(&copy).expect("copy");
  assert_eq!(duplicate.name, original.name);
  assert_eq!(duplicate.on_failure, original.on_failure);
  assert_eq!(depends_on(&edit, "create-pull-request"), ["review", "run-checks"]);
  assert_eq!(
    edit.duplicate_step(&key("review")).map(|key| key.as_str().to_owned()),
    Some("review-copy-2".to_owned())
  );
  assert!(edit.validate().is_ok());
  assert_eq!(edit.duplicate_step(&key("missing")), None);
}

#[test]
fn removed_step_hands_its_dependencies_to_the_steps_after_it() {
  let mut edit = starter_edit();
  assert!(edit.remove_step(&key("plan")));
  assert_eq!(depends_on(&edit, "implement"), ["research"]);
  assert!(edit.validate().is_ok());
  assert!(!edit.remove_step(&key("plan")));
}

#[test]
fn removing_a_retry_target_is_reported_until_the_route_changes() {
  let mut edit = starter_edit();
  assert!(edit.remove_step(&key("implement")));
  assert_eq!(
    edit.validate(),
    Err(PipelineError::UnknownRetryTarget {
      step: key("run-checks"),
      target: key("implement"),
    })
  );
  for step in ["run-checks", "review"] {
    edit.update(&key(step), |step| step.on_failure = None);
  }
  assert!(edit.validate().is_ok());
}

#[test]
fn dependency_edits_are_validated() {
  let mut edit = starter_edit();
  assert!(edit.set_dependency(&key("research"), &key("review"), true));
  assert!(matches!(edit.validate(), Err(PipelineError::Cycle(_))));
  assert!(edit.set_dependency(&key("research"), &key("review"), false));
  assert!(edit.validate().is_ok());

  assert!(edit.set_dependency(&key("review"), &key("implement"), false));
  assert_eq!(
    edit.validate(),
    Err(PipelineError::RetryTargetNotUpstream {
      step: key("review"),
      target: key("implement"),
    })
  );
  assert!(!edit.set_dependency(&key("missing"), &key("review"), true));
}

#[test]
fn update_keeps_the_step_key() {
  let mut edit = starter_edit();
  edit.update(&key("review"), |step| {
    step.key = key("renamed");
    step.name = name("Independent review");
  });
  let review = edit.step(&key("review")).expect("key unchanged");
  assert_eq!(review.name.as_str(), "Independent review");
  assert!(edit.step(&key("renamed")).is_none());
}

#[test]
fn removing_every_step_leaves_a_draft_that_cannot_be_published() {
  let pipeline = Pipeline::new(feature_delivery(text, assigned()).steps()[..1].to_vec()).expect("one step");
  let mut edit = PipelineEdit::new(pipeline);
  assert!(edit.remove_step(&key("research")));
  let empty = edit.validate().expect("an empty draft can be saved");
  assert_eq!(empty.check_publishable(), Err(PublishError::NoSteps));
  assert_eq!(edit.problems(), [EditProblem::NoSteps]);
}

#[test]
fn problems_name_the_cycle_and_every_step_without_a_profile() {
  let mut edit = starter_edit();
  edit.update(&key("review"), |step| {
    step.kind = StepKind::Agent {
      profile: None,
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    };
  });
  assert!(edit.set_dependency(&key("plan"), &key("review"), true));
  let problems = edit.problems();
  assert_eq!(
    problems,
    [
      EditProblem::Cycle(["plan", "implement", "review"].map(key).to_vec()),
      EditProblem::Unassigned(key("review")),
    ]
  );
  assert!(problems[0].blocks_saving());
  assert!(!problems[1].blocks_saving());
}

#[test]
fn description_is_kept_with_the_steps() {
  let mut edit = starter_edit();
  edit.set_description("From a clear brief to a reviewed pull request.");
  let pipeline = edit.validate().expect("valid");
  assert_eq!(pipeline.description(), "From a clear brief to a reviewed pull request.");
  assert_eq!(PipelineEdit::new(pipeline).description(), edit.description());
}

#[test]
fn changes_list_edited_added_and_removed_steps() {
  let published = feature_delivery(text, assigned());
  let mut edit = PipelineEdit::new(published.clone());
  edit.update(&key("review"), |step| {
    step.instructions = PromptText::new("Review the diff.").expect("valid prompt");
    step.on_failure = step.on_failure.clone().map(|route| FailureRoute {
      max_retries: NonZeroU32::new(3).expect("positive"),
      ..route
    });
    step.budget = Some(TokenBudget::new(400_000).expect("above zero"));
  });
  edit.remove_step(&key("research"));
  let added = edit.add_step(name("Audit"), Some(&key("review")));
  let draft = edit.validate().expect("valid");
  let listed: Vec<(String, StepChange)> = changes(&published, &draft)
    .into_iter()
    .map(|change| match change {
      PipelineChange::Step { key, change, .. } => (key.as_str().to_owned(), change),
      PipelineChange::Description => ("description".to_owned(), StepChange::Instructions),
      PipelineChange::Deliver(_) | PipelineChange::Delivers { .. } => {
        panic!("the edit leaves the Deliver step and what runs deliver as they are")
      }
    })
    .collect();
  assert_eq!(
    listed,
    [
      ("plan".to_owned(), StepChange::Dependencies),
      ("review".to_owned(), StepChange::Instructions),
      ("review".to_owned(), StepChange::MaxRetries { from: 2, to: 3 }),
      (
        "review".to_owned(),
        StepChange::Budget {
          from: None,
          to: Some(TokenBudget::new(400_000).expect("above zero"))
        }
      ),
      (added.as_str().to_owned(), StepChange::Added),
      ("research".to_owned(), StepChange::Removed),
    ]
  );
  assert!(changes(&published, &published).is_empty());
}
