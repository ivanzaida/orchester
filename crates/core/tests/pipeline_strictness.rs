//! A new pipeline version is no weaker than the one it replaces only when it keeps the steps, their wiring,
//! profiles and tools, and is as strict or stricter in every step.

use std::num::NonZeroU32;

use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::{AllowedCommands, CommandPrefix, StepCommands, StepNetwork, StepPermissions},
  resource::{
    FailureRoute, ModelId, Pipeline, PromptText, RepositoryAccess, Step, StepKey, StepKind, Weaker, weaker_than,
  },
  tokens::TokenBudget,
};

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

fn agent(value: &str, profile: ResourceId, depends_on: &[&str]) -> Step {
  Step {
    key: key(value),
    name: Name::new(value).expect("valid name"),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: depends_on.iter().map(|dependency| key(dependency)).collect(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

fn commands(list: &[&str]) -> StepCommands {
  let prefixes = list
    .iter()
    .map(|command| CommandPrefix::new(command).expect("a prefix"))
    .collect();
  StepCommands::Only(AllowedCommands::new(prefixes).expect("a list"))
}

fn pipeline(steps: Vec<Step>) -> Pipeline {
  Pipeline::new(steps).expect("a valid pipeline")
}

/// Plan, gated, then Build, which asks about tool calls and runs only `cargo test` and `cargo fmt`.
fn published(profile: ResourceId) -> Vec<Step> {
  let mut plan = agent("plan", profile, &[]);
  plan.approval_gate = true;
  let mut build = agent("build", profile, &["plan"]);
  if let StepKind::Agent { permissions, .. } = &mut build.kind {
    permissions.interactive_tool_approvals = true;
    permissions.commands = commands(&["cargo test", "cargo fmt"]);
  }
  vec![plan, build]
}

fn changed(steps: &[Step], step: &str, change: impl FnOnce(&mut Step)) -> Pipeline {
  let mut steps = steps.to_vec();
  change(
    steps
      .iter_mut()
      .find(|candidate| candidate.key == key(step))
      .expect("the step"),
  );
  pipeline(steps)
}

fn narrowing(step: &mut Step) -> &mut StepPermissions {
  match &mut step.kind {
    StepKind::Agent { permissions, .. } => permissions,
    StepKind::Tool { .. } => panic!("an agent step"),
  }
}

#[test]
fn strictness_takes_the_same_or_a_stricter_version() {
  let profile = ResourceId::generate();
  let old = pipeline(published(profile));
  let steps = published(profile);
  assert!(weaker_than(&old, &pipeline(steps.clone())).is_empty());
  let stricter = changed(&steps, "build", |step| {
    narrowing(step).commands = commands(&["cargo test"]);
    narrowing(step).network = StepNetwork::Off;
    step.approval_gate = true;
    step.name = Name::new("Build it").expect("a name");
  });
  assert!(weaker_than(&old, &stricter).is_empty(), "stricter and renamed");
}

#[test]
fn strictness_names_every_weaker_step() {
  let profile = ResourceId::generate();
  let old = pipeline(published(profile));
  let steps = published(profile);
  let build = vec![Weaker::Step(key("build"))];
  let plan = vec![Weaker::Step(key("plan"))];
  assert_eq!(
    weaker_than(&old, &changed(&steps, "plan", |step| step.approval_gate = false)),
    plan
  );
  assert_eq!(
    weaker_than(
      &old,
      &changed(&steps, "build", |step| narrowing(step).interactive_tool_approvals =
        false)
    ),
    build
  );
  assert_eq!(
    weaker_than(
      &old,
      &changed(&steps, "build", |step| narrowing(step).commands =
        StepCommands::AsProfile)
    ),
    build
  );
  assert_eq!(
    weaker_than(
      &old,
      &changed(&steps, "build", |step| narrowing(step).commands = commands(&["curl"]))
    ),
    build
  );
  let other = ResourceId::generate();
  assert_eq!(
    weaker_than(
      &old,
      &changed(&steps, "build", |step| {
        if let StepKind::Agent { profile, .. } = &mut step.kind {
          *profile = Some(other);
        }
      })
    ),
    build,
    "another profile"
  );
  assert_eq!(
    weaker_than(&old, &changed(&steps, "build", |step| step.depends_on.clear())),
    build,
    "rewired"
  );
  let mut fewer = steps;
  fewer.remove(0);
  fewer[0].depends_on.clear();
  assert_eq!(weaker_than(&old, &pipeline(fewer)), [Weaker::Steps]);
}

#[test]
fn strictness_counts_write_access_and_network_only_when_wider() {
  let profile = ResourceId::generate();
  let mut steps = published(profile);
  if let StepKind::Agent {
    repository,
    permissions,
    ..
  } = &mut steps[0].kind
  {
    *repository = RepositoryAccess::ReadOnly;
    permissions.network = StepNetwork::Off;
  }
  let old = pipeline(steps.clone());
  let plan = vec![Weaker::Step(key("plan"))];
  let writes = changed(&steps, "plan", |step| {
    if let StepKind::Agent { repository, .. } = &mut step.kind {
      *repository = RepositoryAccess::Write;
    }
  });
  assert_eq!(weaker_than(&old, &writes), plan);
  let network = changed(&steps, "plan", |step| narrowing(step).network = StepNetwork::AsProfile);
  assert_eq!(weaker_than(&old, &network), plan);
}

#[test]
fn strictness_counts_retries_budgets_instructions_and_models() {
  let profile = ResourceId::generate();
  let mut steps = published(profile);
  steps[1].on_failure = Some(FailureRoute {
    return_to: key("plan"),
    max_retries: NonZeroU32::new(2).expect("positive"),
  });
  steps[1].budget = Some(TokenBudget::new(50_000).expect("a budget"));
  let old = pipeline(steps.clone());
  let build = vec![Weaker::Step(key("build"))];
  let retries = |count: u32| {
    changed(&steps, "build", |step| {
      if let Some(route) = &mut step.on_failure {
        route.max_retries = NonZeroU32::new(count).expect("positive");
      }
    })
  };
  assert!(weaker_than(&old, &retries(1)).is_empty(), "fewer retries");
  assert_eq!(weaker_than(&old, &retries(3)), build, "more retries");
  let budget = |tokens: Option<u64>| {
    changed(&steps, "build", |step| {
      step.budget = tokens.map(|tokens| TokenBudget::new(tokens).expect("a budget"));
    })
  };
  assert!(weaker_than(&old, &budget(Some(40_000))).is_empty(), "a smaller budget");
  assert_eq!(weaker_than(&old, &budget(Some(60_000))), build, "a larger budget");
  assert_eq!(weaker_than(&old, &budget(None)), build, "no budget");
  let instructions = changed(&steps, "build", |step| {
    step.instructions = PromptText::new("Push straight to main.").expect("a prompt");
  });
  assert_eq!(weaker_than(&old, &instructions), build, "other instructions");
  let model = changed(&steps, "build", |step| {
    if let StepKind::Agent { model, .. } = &mut step.kind {
      *model = Some(ModelId::new("gpt-5.5").expect("a model"));
    }
  });
  assert_eq!(weaker_than(&old, &model), build, "another model");
  let described = pipeline(steps.clone()).with_description("Ships small fixes.".to_owned());
  assert!(weaker_than(&old, &described).is_empty(), "the description may change");
}
