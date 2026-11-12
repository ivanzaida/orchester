//! The flow the Pipelines hub's panel draws for a pipeline's steps (`Pipelines · v2` P1 `s7LMyQ`, P8 `vujkk`;
//! PV2-05): the rail with a node per step, the cards, the approval gate, the group that runs in parallel, where a
//! failure goes, what a card that cannot run offers, and what stands between the start and the end when there is
//! nothing to draw. `cargo test -p orchester-app --test agents pipeline_flow`.

use std::num::NonZeroU32;

use orchester_app::routes::{HubFilter, HubView, PipelineView, PipelinesFilter, PipelinesView, ProjectPage, Route};
use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{
    CommandAction, CommandProgram, DeliverMode, DeliverStep, FailureRoute, MergeMethod, Pipeline, PipelineEdit,
    RepositoryAccess, Slug, StepKey, StepKind, Tool, ToolAction,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{harness::Agents, support::project_id};

pub(super) fn agent(profile: Option<ResourceId>, repository: RepositoryAccess) -> StepKind {
  StepKind::Agent {
    profile,
    repository,
    model: None,
    permissions: StepPermissions::default(),
  }
}

/// Adds the step `name` that waits for `after`, and returns its key.
pub(super) fn add(edit: &mut PipelineEdit, name: &str, after: &[&StepKey], kind: StepKind) -> StepKey {
  let key = edit.add_step(Name::new(name).expect("name"), None);
  let after: Vec<StepKey> = after.iter().map(|key| (*key).clone()).collect();
  edit.update(&key, |step| {
    step.kind = kind;
    step.depends_on = after.into_iter().collect();
  });
  key
}

pub(super) fn fails_back_to(edit: &mut PipelineEdit, step: &StepKey, target: &StepKey) {
  edit.update(step, |step| {
    step.on_failure = Some(FailureRoute {
      return_to: target.clone(),
      max_retries: NonZeroU32::new(2).expect("two"),
    });
  });
}

/// Pipeline `name` created as its first version `body` (a pipeline is stored only as published versions, D-032).
pub(super) fn store(agents: &Agents, name: &str, body: &Pipeline) -> ResourceId {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  db.create_published(
    Slug::from_name(name).expect("slug"),
    Name::new(name).expect("name"),
    body,
    Timestamp::now(),
  )
  .expect("the pipeline")
  .0
  .id()
}

/// A command tool the steps can run, published or only a draft.
pub(super) fn tool(agents: &Agents, published: bool) -> ResourceId {
  let body = Tool::new(
    "",
    ToolAction::Command(CommandAction::new(
      CommandProgram::new("cargo").expect("program"),
      Vec::new(),
    )),
    None,
  )
  .expect("tool");
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = db
    .create_resource(
      Slug::new("checks").expect("slug"),
      Name::new("Checks").expect("name"),
      &body,
      Timestamp::now(),
    )
    .expect("the tool")
    .id();
  if published {
    db.publish_draft::<Tool>(id, Timestamp::now()).expect("published");
  }
  id
}

/// Opens the hub on `pipeline` and waits for its flow.
pub(super) fn open(agents: &mut Agents, pipeline: ResourceId) {
  agents.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(pipeline))),
  ));
  agents.until("the flow", |s| s.exists("pipeline-hub-flow"));
}

#[derive(Clone, Copy, Debug)]
struct Rect {
  y: f32,
  width: f32,
  height: f32,
}

fn rect(agents: &mut Agents, id: &str) -> Rect {
  let found = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|e| e.bounds())
    .unwrap_or_else(|| panic!("{id} laid out"));
  Rect {
    y: found.y,
    width: found.width,
    height: found.height,
  }
}

/// research → plan (gate) → { build ∥ review ∥ checks } → deliver; the review and the checks go back to the plan.
fn feature(agents: &Agents) -> (ResourceId, [StepKey; 5]) {
  let builder = agents.profile(true);
  let checks = tool(agents, true);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let research = add(
    &mut edit,
    "Research",
    &[],
    agent(Some(builder), RepositoryAccess::ReadOnly),
  );
  let plan = add(
    &mut edit,
    "Plan",
    &[&research],
    agent(Some(builder), RepositoryAccess::ReadOnly),
  );
  edit.update(&plan, |step| step.approval_gate = true);
  let build = add(
    &mut edit,
    "Build",
    &[&plan],
    agent(Some(builder), RepositoryAccess::Write),
  );
  let review = add(
    &mut edit,
    "Review",
    &[&plan],
    agent(Some(builder), RepositoryAccess::ReadOnly),
  );
  let run = add(&mut edit, "Run checks", &[&plan], StepKind::Tool { tool: Some(checks) });
  fails_back_to(&mut edit, &review, &plan);
  fails_back_to(&mut edit, &run, &plan);
  let body = edit.validate().expect("a pipeline").with_deliver(Some(DeliverStep::new(
    DeliverMode::AutoMerge,
    MergeMethod::Squash,
    None,
    true,
  )));
  (
    store(agents, "Feature delivery", &body),
    [research, plan, build, review, run],
  )
}

#[test]
fn pipeline_flow_draws_the_start_a_node_per_step_the_gate_the_group_and_the_end_in_order() {
  let mut agents = Agents::new(true);
  let (id, [research, plan, build, review, run]) = feature(&agents);
  open(&mut agents, id);

  for card in [&research, &plan, &build, &review, &run] {
    assert!(
      agents.exists(&format!("pipeline-hub-flow-step-{card}")),
      "a card for {card}"
    );
  }
  assert!(agents.exists("pipeline-hub-flow-row-start-node"), "the start's node");
  assert!(
    agents.exists("pipeline-hub-flow-row-1-node"),
    "research's node, numbered 1"
  );
  assert!(agents.exists("pipeline-hub-flow-row-2-node"), "plan's node, numbered 2");
  assert!(
    agents.exists("pipeline-hub-flow-gate-100"),
    "the approval gate after the plan"
  );
  assert!(agents.exists("pipeline-hub-flow-row-gate-1-0-node"), "the gate's node");
  assert!(
    agents.exists("pipeline-hub-flow-parallel-3"),
    "the three steps that wait for the plan stand in one group"
  );
  assert!(agents.exists("pipeline-hub-flow-row-3-node"), "the group's node");
  assert!(agents.exists("pipeline-hub-flow-deliver"), "the Deliver card");
  assert_eq!(agents.text(&format!("pipeline-hub-flow-step-{run}-number")), "05");

  let order = [
    "pipeline-hub-flow-row-start",
    "pipeline-hub-flow-row-1",
    "pipeline-hub-flow-row-2",
    "pipeline-hub-flow-row-gate-1-0",
    "pipeline-hub-flow-row-3",
    "pipeline-hub-flow-row-deliver",
    "pipeline-hub-flow-row-end",
  ];
  let ys: Vec<f32> = order.iter().map(|id| rect(&mut agents, id).y).collect();
  assert!(
    ys.windows(2).all(|pair| pair[0] < pair[1]),
    "start, steps, gate, group, deliver, end run down the line: {ys:?}"
  );
}

#[test]
fn pipeline_flow_a_node_is_22_px_across_as_the_design_draws_it() {
  let mut agents = Agents::new(true);
  let (id, _) = feature(&agents);
  open(&mut agents, id);
  for node in [
    "pipeline-hub-flow-row-start-node",
    "pipeline-hub-flow-row-1-node",
    "pipeline-hub-flow-row-end-node",
    "pipeline-hub-flow-row-gate-1-0-node",
  ] {
    let found = rect(&mut agents, node);
    assert_eq!((found.width, found.height), (22.0, 22.0), "{node}");
  }
}

#[test]
fn pipeline_flow_pressing_a_card_opens_that_step_and_the_deliver_card_opens_deliver() {
  let mut agents = Agents::new(true);
  let (id, [_, plan, ..]) = feature(&agents);
  open(&mut agents, id);
  agents.click(&format!("pipeline-hub-flow-step-{plan}"));
  agents.until("the step", |s| {
    s.window.navigation().route_now()
      == Route::Project(
        project_id(),
        ProjectPage::Pipeline(id, PipelineView::Step(plan.clone())),
      )
  });

  open(&mut agents, id);
  agents.click("pipeline-hub-flow-deliver");
  agents.until("deliver", |s| {
    s.window.navigation().route_now() == Route::Project(project_id(), ProjectPage::Pipeline(id, PipelineView::Deliver))
  });
}

#[test]
fn pipeline_flow_a_failure_route_reads_by_what_the_step_is() {
  let mut agents = Agents::new(true);
  let (id, [_, _, _, review, run]) = feature(&agents);
  open(&mut agents, id);
  assert_eq!(
    agents.text(&format!("pipeline-hub-flow-step-{review}-failure-words")),
    "changes requested → 02",
    "an agent step asks for changes"
  );
  assert_eq!(
    agents.text(&format!("pipeline-hub-flow-step-{run}-failure-words")),
    "fails → 02",
    "a tool step fails"
  );
}

#[test]
fn pipeline_flow_steps_count_the_steps_and_never_deliver_in_the_panel_and_the_list() {
  let mut agents = Agents::new(true);
  let (id, _) = feature(&agents);
  open(&mut agents, id);
  assert_eq!(
    agents.text("pipeline-hub-steps-count"),
    "5",
    "five steps and a Deliver card"
  );
  assert_eq!(agents.text("pipeline-hub-flow-deliver-name"), "Deliver");
  assert!(
    agents
      .text(&format!("pipeline-hub-row-{id}-line"))
      .starts_with("5 steps"),
    "the list row counts the same"
  );
}

#[test]
fn pipeline_flow_the_cards_of_a_group_share_one_baseline_with_or_without_a_pill() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let checks = tool(&agents, true);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let first = add(&mut edit, "Build", &[], agent(Some(builder), RepositoryAccess::Write));
  let review = add(
    &mut edit,
    "Review",
    &[&first],
    agent(Some(builder), RepositoryAccess::ReadOnly),
  );
  let run = add(
    &mut edit,
    "Run checks",
    &[&first],
    StepKind::Tool { tool: Some(checks) },
  );
  let id = store(&agents, "Group", &edit.validate().expect("a pipeline"));
  open(&mut agents, id);
  assert!(
    agents.exists(&format!("pipeline-hub-flow-step-{review}-access")),
    "the review has a pill"
  );
  assert!(
    !agents.exists(&format!("pipeline-hub-flow-step-{run}-access")),
    "the checks have none"
  );
  let with = rect(&mut agents, &format!("pipeline-hub-flow-step-{review}-name"));
  let without = rect(&mut agents, &format!("pipeline-hub-flow-step-{run}-name"));
  assert_eq!(with.y, without.y, "both titles stand on one baseline");
}

#[test]
fn pipeline_flow_a_card_that_cannot_run_offers_the_profile_or_the_tool_it_names() {
  let mut agents = Agents::new(true);
  let draft_profile = agents.profile(false);
  let draft_tool = tool(&agents, false);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let plan = add(
    &mut edit,
    "Plan",
    &[],
    agent(Some(draft_profile), RepositoryAccess::ReadOnly),
  );
  let run = add(
    &mut edit,
    "Run checks",
    &[&plan],
    StepKind::Tool { tool: Some(draft_tool) },
  );
  let id = store(&agents, "Unready", &edit.validate().expect("a pipeline"));
  open(&mut agents, id);

  assert_eq!(
    agents.text(&format!("pipeline-hub-flow-step-{plan}-fix-choose-words")),
    "Open profile"
  );
  assert_eq!(
    agents.text(&format!("pipeline-hub-flow-step-{run}-fix-choose-words")),
    "Open tool"
  );
  agents.click(&format!("pipeline-hub-flow-step-{plan}-fix"));
  agents.until("the profile", |s| {
    s.window.navigation().route_now()
      == Route::Project(
        project_id(),
        ProjectPage::AgentsHub(HubView::new(HubFilter::Profiles, Some(draft_profile))),
      )
  });

  open(&mut agents, id);
  agents.click(&format!("pipeline-hub-flow-step-{run}-fix"));
  agents.until("the tool", |s| {
    s.window.navigation().route_now()
      == Route::Project(
        project_id(),
        ProjectPage::AgentsHub(HubView::new(HubFilter::Tools, Some(draft_tool))),
      )
  });
}

#[test]
fn pipeline_flow_deliver_names_a_profile_that_is_not_published_and_warns_instead_of_saying_you() {
  let mut agents = Agents::new(true);
  let draft_profile = agents.profile(false);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  add(
    &mut edit,
    "Build",
    &[],
    agent(Some(draft_profile), RepositoryAccess::Write),
  );
  let body = edit.validate().expect("a pipeline").with_deliver(Some(DeliverStep::new(
    DeliverMode::AutoMerge,
    MergeMethod::Squash,
    None,
    true,
  )));
  let id = store(&agents, "Merging", &body);
  open(&mut agents, id);
  assert_eq!(
    agents.text("pipeline-hub-flow-deliver-line"),
    "Auto-merge · Squash · conflicts: Builder (not published)"
  );
}

#[test]
fn pipeline_flow_steps_that_cannot_be_drawn_as_one_line_keep_the_start_and_end_and_say_so_between_them() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let a = add(&mut edit, "A", &[], agent(Some(builder), RepositoryAccess::ReadOnly));
  add(&mut edit, "B", &[&a], agent(Some(builder), RepositoryAccess::ReadOnly));
  let c = add(&mut edit, "C", &[], agent(Some(builder), RepositoryAccess::ReadOnly));
  add(&mut edit, "D", &[&c], agent(Some(builder), RepositoryAccess::ReadOnly));
  let id = store(&agents, "Chains", &edit.validate().expect("a pipeline"));
  open(&mut agents, id);

  let start = rect(&mut agents, "pipeline-hub-flow-row-start").y;
  let notice = rect(&mut agents, "pipeline-hub-flow-undrawable").y;
  let end = rect(&mut agents, "pipeline-hub-flow-row-end").y;
  assert!(
    start < notice && notice < end,
    "the notice stands between the markers: {start} {notice} {end}"
  );
  assert!(
    !agents.exists("pipeline-hub-edit-steps") && !agents.exists("pipeline-hub-add-step"),
    "the notice has the one Edit steps; the head has no action"
  );
  agents.click("pipeline-hub-flow-undrawable-edit");
  agents.until("the editor", |s| {
    s.window.navigation().route_now()
      == Route::Project(project_id(), ProjectPage::Pipeline(id, PipelineView::default()))
  });
}

#[test]
fn pipeline_flow_a_pipeline_draws_its_latest_versions_steps() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let first = add(&mut edit, "Plan", &[], agent(Some(builder), RepositoryAccess::ReadOnly));
  let id = store(&agents, "Evolving", &edit.validate().expect("a pipeline"));
  let second = add(
    &mut edit,
    "Build",
    &[&first],
    agent(Some(builder), RepositoryAccess::Write),
  );
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let latest = db
    .latest_version::<Pipeline>(id)
    .expect("read")
    .expect("a version")
    .number();
  db.publish_pipeline_edit(
    id,
    &edit.validate().expect("a pipeline"),
    latest,
    None,
    Timestamp::now(),
  )
  .expect("v2");

  open(&mut agents, id);
  agents.until("the latest version's steps", |s| {
    s.exists(&format!("pipeline-hub-flow-step-{second}"))
  });
  assert!(agents.exists(&format!("pipeline-hub-flow-step-{first}")));
  assert_eq!(agents.text("pipeline-hub-steps-count"), "2");
}

#[test]
fn pipeline_flow_the_deliver_card_has_a_node_of_its_own_and_no_number() {
  let mut agents = Agents::new(true);
  let (id, _) = feature(&agents);
  open(&mut agents, id);

  let node = rect(&mut agents, "pipeline-hub-flow-row-deliver-node");
  assert_eq!((node.width, node.height), (22.0, 22.0), "a node like the others");
  assert!(
    !agents.exists("pipeline-hub-flow-row-deliver-number"),
    "Deliver is not a step, so its node shows a glyph, not 06"
  );
  assert!(
    agents.exists("pipeline-hub-flow-row-1-node") && agents.exists("pipeline-hub-flow-row-1-number"),
    "the steps keep their numbers"
  );
}

#[test]
fn pipeline_flow_a_gate_names_deliver_by_word_and_a_step_by_its_number() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let plan = add(&mut edit, "Plan", &[], agent(Some(builder), RepositoryAccess::ReadOnly));
  edit.update(&plan, |step| step.approval_gate = true);
  let build = add(
    &mut edit,
    "Build",
    &[&plan],
    agent(Some(builder), RepositoryAccess::Write),
  );
  edit.update(&build, |step| step.approval_gate = true);
  let body = edit.validate().expect("a pipeline").with_deliver(Some(DeliverStep::new(
    DeliverMode::AutoMerge,
    MergeMethod::Squash,
    None,
    true,
  )));
  let id = store(&agents, "Gated", &body);
  open(&mut agents, id);

  assert_eq!(
    agents.text("pipeline-hub-flow-gate-0-hint"),
    "you approve Plan before 02 starts"
  );
  assert_eq!(
    agents.text("pipeline-hub-flow-gate-100-hint"),
    "you approve Build before Deliver starts"
  );
}
