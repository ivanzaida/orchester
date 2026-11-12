//! A pipeline as the Pipelines page draws its steps (`Pipelines · v2` P1, P8): the layout comes from the steps'
//! dependencies. `cargo test -p orchester-workspace --test pipeline_flow`.

use std::{num::NonZeroU32, sync::Arc};

use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::{
    DeliverMode, DeliverStep, Deliverable, FailureRoute, FlowError, MergeMethod, ModelId, Pipeline, PromptText,
    RepositoryAccess, Step, StepKey, StepKind, ToolKind, VersionNumber,
  },
};
use orchester_workspace::pipelines::{
  data::{Assignables, DraftProfileChoice, DraftToolChoice, ProfileChoice, ToolChoice},
  flow::{Conflicts, Ending, Named, PipelineFlow, Runs, Waits},
};

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

fn profile(name: &str, model: &str) -> ProfileChoice {
  ProfileChoice {
    id: ResourceId::generate(),
    name: name.into(),
    version: VersionNumber::new(NonZeroU32::MIN),
    model: ModelId::new(model).expect("valid model"),
    // A profile runs on a connection the project has; `None` would say it was removed.
    connection: Some("Claude".into()),
    provider: None,
    permissions: AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::AskBefore,
      NetworkAccess::Off,
    ),
    agent: None,
    mcp: Default::default(),
  }
}

fn tool(name: &str, kind: ToolKind) -> ToolChoice {
  ToolChoice {
    id: ResourceId::generate(),
    name: name.into(),
    version: VersionNumber::new(NonZeroU32::MIN),
    kind,
    command: None,
    runnable: kind.runnable(),
  }
}

fn step(name: &str, kind: StepKind, depends_on: &[&str]) -> Step {
  Step {
    key: key(name),
    name: Name::new(name).expect("valid name"),
    kind,
    instructions: PromptText::new("Work on {{task.title}}.").expect("valid prompt"),
    depends_on: depends_on.iter().map(|dependency| key(dependency)).collect(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

fn agent(profile: &ProfileChoice, access: RepositoryAccess) -> StepKind {
  StepKind::Agent {
    profile: Some(profile.id),
    repository: access,
    model: None,
    permissions: Default::default(),
  }
}

/// research → plan (gate) → implement → { checks ∥ review } → deliver, as P1 draws it.
fn feature() -> (Pipeline, Assignables) {
  let explorer = profile("Explorer", "claude-sonnet-5-5");
  let planner = profile("Planner", "claude-opus-5-5");
  let builder = profile("Builder", "claude-opus-5-5");
  let reviewer = profile("Reviewer", "claude-opus-5-5");
  let checks = tool("Checks", ToolKind::Command);
  let route = |target: &str| FailureRoute {
    return_to: key(target),
    max_retries: NonZeroU32::new(2).expect("two"),
  };
  let mut plan = step("plan", agent(&planner, RepositoryAccess::ReadOnly), &["research"]);
  plan.approval_gate = true;
  let mut run_checks = step("run-checks", StepKind::Tool { tool: Some(checks.id) }, &["implement"]);
  run_checks.on_failure = Some(route("implement"));
  let mut review = step("review", agent(&reviewer, RepositoryAccess::ReadOnly), &["implement"]);
  review.on_failure = Some(route("implement"));
  let steps = vec![
    step("research", agent(&explorer, RepositoryAccess::ReadOnly), &[]),
    plan,
    step("implement", agent(&builder, RepositoryAccess::Write), &["plan"]),
    run_checks,
    review,
  ];
  let pipeline = Pipeline::new(steps)
    .expect("valid steps")
    .with_deliver(Some(DeliverStep::new(
      DeliverMode::ApproveToMerge,
      MergeMethod::Squash,
      None,
      true,
    )));
  let assignables = Assignables {
    profiles: vec![explorer, planner, builder, reviewer],
    tools: vec![checks],
    ..Assignables::default()
  };
  (pipeline, assignables)
}

fn names(flow: &PipelineFlow) -> Vec<Vec<(usize, &str)>> {
  flow
    .stages
    .iter()
    .map(|stage| stage.cards.iter().map(|card| (card.number, &*card.name)).collect())
    .collect()
}

#[test]
fn the_cards_are_numbered_stage_by_stage_with_parallel_steps_in_one_stage() {
  let (pipeline, assignables) = feature();
  let flow = PipelineFlow::of(&pipeline, &assignables).expect("drawable");
  assert_eq!(
    names(&flow),
    [
      vec![(1, "research")],
      vec![(2, "plan")],
      vec![(3, "implement")],
      vec![(4, "run-checks"), (5, "review")],
    ]
  );
  assert_eq!(flow.deliver.as_ref().map(|deliver| deliver.number), Some(6));
  assert_eq!(
    flow.len(),
    5,
    "the steps: Deliver is numbered after them but is not one"
  );
  assert_eq!(flow.len(), pipeline.steps().len());
}

#[test]
fn a_card_says_what_it_runs_and_what_it_may_do_to_the_repository() {
  let (pipeline, assignables) = feature();
  let flow = PipelineFlow::of(&pipeline, &assignables).expect("drawable");
  let card = |stage: usize, index: usize| &flow.stages[stage].cards[index];
  assert_eq!(
    card(0, 0).runs,
    Runs::Profile {
      name: "Explorer".into(),
      model: "claude-sonnet-5-5".into()
    }
  );
  assert_eq!(card(0, 0).access, Some(RepositoryAccess::ReadOnly));
  assert_eq!(card(2, 0).access, Some(RepositoryAccess::Write));
  assert_eq!(card(3, 0).runs, Runs::Tool { name: "Checks".into() });
  assert_eq!(card(3, 0).access, None);
}

#[test]
fn a_step_that_overrides_the_model_shows_it() {
  let (pipeline, assignables) = feature();
  let builder = assignables.profile_named("Builder").expect("builder");
  let steps: Vec<Step> = pipeline
    .steps()
    .iter()
    .map(|step| match &step.kind {
      StepKind::Agent { profile, .. } if *profile == Some(builder) => Step {
        kind: StepKind::Agent {
          profile: Some(builder),
          repository: RepositoryAccess::Write,
          model: Some(ModelId::new("claude-haiku-5-5").expect("valid model")),
          permissions: Default::default(),
        },
        ..step.clone()
      },
      _ => step.clone(),
    })
    .collect();
  let pipeline = Pipeline::new(steps).expect("valid steps");
  let flow = PipelineFlow::of(&pipeline, &assignables).expect("drawable");
  assert_eq!(
    flow.stages[2].cards[0].runs,
    Runs::Profile {
      name: "Builder".into(),
      model: "claude-haiku-5-5".into()
    }
  );
}

#[test]
fn a_gate_stands_after_its_step_and_names_what_runs_next() {
  let (pipeline, assignables) = feature();
  let flow = PipelineFlow::of(&pipeline, &assignables).expect("drawable");
  assert_eq!(flow.stages[1].gates.len(), 1);
  assert_eq!(&*flow.stages[1].gates[0].step, "plan");
  assert_eq!(flow.stages[1].gates[0].before, Some(3));
  assert!(
    flow
      .stages
      .iter()
      .enumerate()
      .all(|(index, stage)| index == 1 || stage.gates.is_empty())
  );
}

#[test]
fn a_gate_on_the_last_step_waits_for_the_deliver_step_or_for_the_end() {
  let (pipeline, assignables) = feature();
  let gated = |mut step: Step| {
    step.approval_gate = true;
    step
  };
  let steps: Vec<Step> = pipeline.steps().iter().cloned().map(gated).collect();
  let with_deliver = Pipeline::new(steps.clone())
    .expect("valid steps")
    .with_deliver(pipeline.deliver().cloned());
  let flow = PipelineFlow::of(&with_deliver, &assignables).expect("drawable");
  let last = flow.stages.last().expect("stages");
  assert_eq!(last.gates.len(), 2, "one gate per gated step of the group");
  assert!(last.gates.iter().all(|gate| gate.before == Some(6)));

  let without_deliver = Pipeline::new(steps).expect("valid steps");
  let flow = PipelineFlow::of(&without_deliver, &assignables).expect("drawable");
  assert!(flow.deliver.is_none());
  assert!(
    flow
      .stages
      .last()
      .expect("stages")
      .gates
      .iter()
      .all(|gate| gate.before.is_none())
  );
}

#[test]
fn a_failure_route_names_the_card_it_returns_to_by_its_number() {
  let (pipeline, assignables) = feature();
  let flow = PipelineFlow::of(&pipeline, &assignables).expect("drawable");
  for card in &flow.stages[3].cards {
    let route = card.on_failure.as_ref().expect("returns on failure");
    assert_eq!((route.number, &*route.name), (3, "implement"));
  }
  assert!(flow.stages[2].cards[0].on_failure.is_none());
}

#[test]
fn the_deliver_card_says_how_it_merges_and_who_resolves_conflicts() {
  let (pipeline, assignables) = feature();
  let flow = PipelineFlow::of(&pipeline, &assignables).expect("drawable");
  let deliver = flow.deliver.expect("deliver step");
  assert_eq!(deliver.mode, DeliverMode::ApproveToMerge);
  assert_eq!(deliver.method, MergeMethod::Squash);
  assert_eq!(deliver.conflicts, Conflicts::Profile(Arc::from("Builder")));
}

#[test]
fn conflicts_go_to_the_person_when_no_step_can_write() {
  let (pipeline, assignables) = feature();
  let reader_only: Vec<Step> = pipeline
    .steps()
    .iter()
    .map(|step| match &step.kind {
      StepKind::Agent {
        profile,
        model,
        permissions,
        ..
      } => Step {
        kind: StepKind::Agent {
          profile: *profile,
          repository: RepositoryAccess::ReadOnly,
          model: model.clone(),
          permissions: permissions.clone(),
        },
        ..step.clone()
      },
      StepKind::Tool { .. } => step.clone(),
    })
    .collect();
  let pipeline = Pipeline::new(reader_only)
    .expect("valid steps")
    .with_deliver(pipeline.deliver().cloned());
  let flow = PipelineFlow::of(&pipeline, &assignables).expect("drawable");
  assert_eq!(flow.deliver.expect("deliver step").conflicts, Conflicts::Person);
}

#[test]
fn a_pipeline_without_a_deliver_step_ends_in_what_it_delivers() {
  let (pipeline, assignables) = feature();
  let report = Pipeline::new(pipeline.steps().to_vec())
    .expect("valid steps")
    .with_delivers(Deliverable::Report);
  let flow = PipelineFlow::of(&report, &assignables).expect("drawable");
  assert!(flow.deliver.is_none());
  assert_eq!(Ending::of(&report), Ending::Delivers(Deliverable::Report));
  assert_eq!(Ending::of(&pipeline), Ending::Merged);
  assert_eq!(flow.len(), 5);
}

#[test]
fn steps_without_a_profile_or_tool_or_with_an_unpublished_one_need_you() {
  let someone = ResourceId::generate();
  let steps = vec![
    step(
      "a",
      StepKind::Agent {
        profile: None,
        repository: RepositoryAccess::ReadOnly,
        model: None,
        permissions: Default::default(),
      },
      &[],
    ),
    step("b", StepKind::Tool { tool: None }, &["a"]),
    step(
      "c",
      StepKind::Agent {
        profile: Some(someone),
        repository: RepositoryAccess::ReadOnly,
        model: None,
        permissions: Default::default(),
      },
      &["b"],
    ),
    step("d", StepKind::Tool { tool: Some(someone) }, &["c"]),
  ];
  let pipeline = Pipeline::new(steps).expect("valid steps");
  let flow = PipelineFlow::of(&pipeline, &Assignables::default()).expect("drawable");
  let runs: Vec<&Runs> = flow.stages.iter().map(|stage| &stage.cards[0].runs).collect();
  assert_eq!(
    runs,
    [
      &Runs::NoProfile,
      &Runs::NoTool,
      &Runs::ProfileNotPublished { id: someone },
      &Runs::ToolNotPublished { id: someone }
    ]
  );
  assert!(runs.iter().all(|runs| runs.needs_you()));
  assert!(Runs::NoProfile.is_unassigned() && !Runs::ProfileNotPublished { id: someone }.is_unassigned());
}

#[test]
fn a_tool_a_step_cannot_run_yet_needs_you() {
  let http = tool("Webhook", ToolKind::Http);
  let http_id = http.id;
  let steps = vec![step("a", StepKind::Tool { tool: Some(http.id) }, &[])];
  let assignables = Assignables {
    tools: vec![http],
    ..Assignables::default()
  };
  let flow = PipelineFlow::of(&Pipeline::new(steps).expect("valid steps"), &assignables).expect("drawable");
  let runs = &flow.stages[0].cards[0].runs;
  assert_eq!(
    runs,
    &Runs::ToolNotRunnable {
      id: http_id,
      name: Arc::from("Webhook")
    }
  );
  assert!(runs.needs_you());
}

#[test]
fn independent_chains_are_drawn_by_stage_and_each_card_says_what_it_waits_for() {
  let (_, assignables) = feature();
  let tool_step = |name: &str, depends_on: &[&str]| step(name, StepKind::Tool { tool: None }, depends_on);
  let steps = vec![
    tool_step("a", &[]),
    tool_step("b", &["a"]),
    tool_step("c", &[]),
    tool_step("d", &["c"]),
  ];
  let pipeline = Pipeline::new(steps).expect("valid steps");
  let flow = PipelineFlow::of(&pipeline, &assignables).expect("chains are drawn");
  let stages: Vec<Vec<(&str, usize)>> = flow
    .stages
    .iter()
    .map(|stage| {
      stage
        .cards
        .iter()
        .map(|card| (card.key.as_str(), card.number))
        .collect()
    })
    .collect();
  assert_eq!(stages, [vec![("a", 1), ("c", 2)], vec![("b", 3), ("d", 4)]]);
  let b = &flow.stages[1].cards[0];
  assert_eq!(
    b.waits,
    Some(Waits {
      on: vec![Named {
        number: 1,
        name: "a".into()
      }],
      not_on: vec![Named {
        number: 2,
        name: "c".into()
      }],
    })
  );
  assert!(flow.stages[0].cards.iter().all(|card| card.waits.is_none()));
}

#[test]
fn a_step_that_waits_for_the_whole_stage_before_has_no_waits_line() {
  let (pipeline, assignables) = feature();
  let flow = PipelineFlow::of(&pipeline, &assignables).expect("the feature flow");
  assert!(
    flow
      .stages
      .iter()
      .flat_map(|stage| &stage.cards)
      .all(|card| card.waits.is_none())
  );
}

#[test]
fn steps_that_wait_for_each_other_have_no_flow() {
  let (_, assignables) = feature();
  let mut edit = orchester_core::resource::PipelineEdit::new(Pipeline::new(Vec::new()).expect("empty"));
  let first = edit.add_step(Name::new("First").expect("name"), None);
  let second = edit.add_step(Name::new("Second").expect("name"), Some(&first));
  edit.set_dependency(&first, &second, true);
  let read = orchester_workspace::pipelines::flow::FlowRead::of_edit(&edit, &assignables);
  assert!(matches!(read.flow, Err(FlowError::Broken(_))));
  let numbers: Vec<usize> = read.unplaced.iter().map(|card| card.number).collect();
  assert_eq!(numbers, [1, 2], "the cards keep the order of the steps");
}

#[test]
fn conflicts_name_a_profile_that_is_not_published_rather_than_the_person() {
  let (pipeline, mut assignables) = feature();
  let position = assignables
    .profiles
    .iter()
    .position(|profile| &*profile.name == "Builder")
    .expect("the builder");
  let builder = assignables.profiles.remove(position);
  assignables.draft_profiles.push(DraftProfileChoice {
    id: builder.id,
    name: builder.name.clone(),
    model: builder.model,
  });
  let flow = PipelineFlow::of(&pipeline, &assignables).expect("drawable");
  assert_eq!(
    flow.deliver.expect("deliver step").conflicts,
    Conflicts::ProfileNotPublished(Arc::from("Builder"))
  );
}

#[test]
fn a_card_knows_whether_its_step_runs_a_tool() {
  let (pipeline, assignables) = feature();
  let flow = PipelineFlow::of(&pipeline, &assignables).expect("drawable");
  let tools: Vec<(usize, bool)> = flow
    .stages
    .iter()
    .flat_map(|stage| &stage.cards)
    .map(|card| (card.number, card.is_tool()))
    .collect();
  assert_eq!(tools, [(1, false), (2, false), (3, false), (4, true), (5, false)]);
}

#[test]
fn a_card_knows_a_profile_or_tool_that_is_gone_from_one_only_not_published() {
  let (archived, draft, draft_tool) = (ResourceId::generate(), ResourceId::generate(), ResourceId::generate());
  let agent_on = |id| StepKind::Agent {
    profile: Some(id),
    repository: RepositoryAccess::ReadOnly,
    model: None,
    permissions: Default::default(),
  };
  let steps = vec![
    step("a", agent_on(archived), &[]),
    step("b", agent_on(draft), &["a"]),
    step("c", StepKind::Tool { tool: Some(draft_tool) }, &["b"]),
    step("d", StepKind::Tool { tool: Some(archived) }, &["c"]),
  ];
  let assignables = Assignables {
    draft_profiles: vec![DraftProfileChoice {
      id: draft,
      name: "Drafted".into(),
      model: ModelId::new("m").expect("valid model"),
    }],
    draft_tools: vec![DraftToolChoice {
      id: draft_tool,
      name: "Drafted tool".into(),
    }],
    ..Assignables::default()
  };
  let flow = PipelineFlow::of(&Pipeline::new(steps).expect("valid steps"), &assignables).expect("drawable");
  let gone: Vec<bool> = flow.stages.iter().map(|stage| stage.cards[0].gone).collect();
  assert_eq!(gone, [true, false, false, true]);
  assert!(matches!(flow.stages[0].cards[0].runs, Runs::ProfileNotPublished { .. }));
}

#[test]
fn a_profile_whose_connection_was_removed_cannot_run_and_says_so() {
  let mut lost = profile("Explorer", "claude-sonnet-5-5");
  lost.connection = None;
  let pipeline = Pipeline::new(vec![step("research", agent(&lost, RepositoryAccess::ReadOnly), &[])]).expect("valid");
  let assignables = Assignables {
    profiles: vec![lost.clone()],
    ..Assignables::default()
  };

  let flow = PipelineFlow::of(&pipeline, &assignables).expect("a flow");
  let runs = &flow.stages[0].cards[0].runs;
  assert_eq!(
    runs,
    &Runs::ProfileConnectionGone {
      id: lost.id,
      name: "Explorer".into()
    }
  );
  assert!(runs.needs_you(), "its card stands in amber");
}
