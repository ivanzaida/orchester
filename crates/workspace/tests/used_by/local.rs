//! A local project computes the answer from its database: the same names rule and order as the contract.

use std::collections::{BTreeMap, BTreeSet};

use orchester_api::SharedResourceKind::{AgentProfile as AgentProfileKind, Pipeline as PipelineKind};
use orchester_core::{
  id::{ConnectionId, IdempotencyKey, ProjectId, ResourceId, TaskId},
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, AgentTargets, AllowedSteps, ApprovalMode, CommandAction, CommandArg,
    CommandProgram, DeliverStep, DeniedBehavior, Draft, InjectionEvent, Instruction, InstructionScope, ModelId,
    Pipeline, PromptText, ProviderSettings, RepositoryAccess, Skill, Slug, Step, StepKey, StepKind, Tool, ToolAction,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot, StepFailure},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::used_by::{UsedBy, UsedByError, used_by};
use tempfile::TempDir;

use super::{repository, shown, tool_users};

fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn pipeline_running(tool: ResourceId) -> Pipeline {
  pipeline_of(StepKind::Tool { tool: Some(tool) })
}

/// A pipeline of the one step `kind`.
fn pipeline_of(kind: StepKind) -> Pipeline {
  let step = Step {
    key: StepKey::new("check").expect("key"),
    name: name("Check"),
    kind,
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  Pipeline::new(vec![step]).expect("pipeline")
}

fn instruction_for(scope: InstructionScope, text: &str) -> Instruction {
  instruction_targeting(AgentTargets::All, scope, text)
}

fn instruction_targeting(targets: AgentTargets, scope: InstructionScope, text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("prompt"),
    targets,
    BTreeSet::from([InjectionEvent::Start]),
    scope,
  )
  .expect("instruction")
}

fn status_action() -> CommandAction {
  CommandAction::new(
    CommandProgram::new("git").expect("program"),
    vec![CommandArg::new("status").expect("arg")],
  )
}

/// A step that an agent of `profile` runs.
fn agent_step(profile: ResourceId) -> StepKind {
  StepKind::Agent {
    profile: Some(profile),
    repository: RepositoryAccess::Write,
    model: None,
    permissions: StepPermissions::default(),
  }
}

/// A local project KON in a folder, with a counter for the timestamps of what it stores.
struct Local {
  dir: TempDir,
  db: ProjectDb,
  clock: i64,
}

impl Local {
  fn new() -> Self {
    let dir = repository();
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("KON").expect("key"),
      name("Kontur"),
    );
    let db = ProjectDb::create(dir.path(), project).expect("project");
    Self { dir, db, clock: 0 }
  }

  fn now(&mut self) -> Timestamp {
    self.clock += 1;
    at(self.clock)
  }

  /// A resource of `body` with its v1 published.
  fn publish<B: orchester_core::resource::ResourceBody>(
    &mut self,
    (slug, label): (&str, &str),
    body: &B,
  ) -> ResourceId {
    let now = self.now();
    let resource = self
      .db
      .create_resource(Slug::new(slug).expect("slug"), name(label), body, now)
      .expect("resource");
    let now = self.now();
    self.db.publish_draft::<B>(resource.id(), now).expect("v1");
    resource.id()
  }

  /// `resource`'s next version, with `body`.
  fn republish<B: orchester_core::resource::ResourceBody>(&mut self, resource: ResourceId, body: B) {
    let latest = self
      .db
      .latest_version::<B>(resource)
      .expect("read")
      .expect("published")
      .number();
    let now = self.now();
    self
      .db
      .save_draft(resource, &Draft::new(body, Some(latest), now))
      .expect("draft");
    let now = self.now();
    self.db.publish_draft::<B>(resource, now).expect("next version");
  }

  fn tool(&mut self, slug: &str, label: &str) -> ResourceId {
    self.publish((slug, label), &Tool::command("", status_action()))
  }

  fn profile(&mut self, slug: &str, label: &str) -> ResourceId {
    let profile = AgentProfile::new(
      name(label),
      String::new(),
      PromptText::new("Build.").expect("prompt"),
      ConnectionId::generate(),
      ModelId::new("gpt-5.5").expect("model"),
      ProviderSettings::default(),
    );
    self.publish((slug, label), &profile)
  }

  /// `count` tasks, so that the later ones have the larger numbers; the ids in order.
  fn tasks(&mut self, count: usize) -> Vec<TaskId> {
    let details = (1..=count)
      .map(|number| TaskDetails {
        title: name(&format!("Task {number}")),
        brief: "Do it.".to_owned(),
        acceptance_criteria: vec!["Done".to_owned()],
        pipeline: None,
        limits: TaskLimits::NONE,
      })
      .collect();
    let now = self.now();
    self
      .db
      .create_tasks(IdempotencyKey::generate(), details, now)
      .expect("tasks")
      .iter()
      .map(orchester_core::task::Task::id)
      .collect()
  }

  /// A run of `task` that pins `pipeline` and the `tool` it runs, started and at work.
  fn run(&mut self, task: TaskId, (pipeline, tool): (ResourceId, ResourceId)) -> Run {
    let pipeline_version = self
      .db
      .latest_version::<Pipeline>(pipeline)
      .expect("read")
      .expect("published");
    let tool_version = self.db.latest_version::<Tool>(tool).expect("read").expect("published");
    let tools = BTreeMap::from([(tool, tool_version)]);
    let published = PublishedVersions {
      profiles: &BTreeMap::new(),
      tools: &tools,
      instructions: &BTreeMap::new(),
      skills: &BTreeMap::new(),
    };
    let snapshot = RunSnapshot::build(pipeline, &pipeline_version, published).expect("snapshot");
    let number = self.db.next_run_number().expect("number");
    let new = NewRun {
      task,
      snapshot,
      branch: BranchName::new(&format!("orchester/kon-run-{number}")).expect("branch"),
      base: Revision::new("0123456789abcdef0123456789abcdef01234567").expect("revision"),
      base_branch: Some(BranchName::new("main").expect("branch")),
      base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
      budget: None,
      max_duration: None,
    };
    let now = self.now();
    let mut run = self.db.create_run(IdempotencyKey::generate(), new, now).expect("run");
    let now = self.now();
    let events = run.start(now).expect("start");
    self.db.append_events(&run, &events).expect("events");
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let now = self.now();
    let events = run.start_step(execution, now).expect("step");
    self.db.append_events(&run, &events).expect("events");
    run
  }

  /// `run`'s step failed: the run ended.
  fn fail(&mut self, run: &mut Run) {
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let now = self.now();
    let events = run
      .fail_step(execution, StepFailure::ReportedFailed, now)
      .expect("fail");
    self.db.append_events(run, &events).expect("events");
  }

  fn used_by(&self, resource: ResourceId) -> Result<UsedBy, UsedByError> {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(used_by(self.dir.path(), None, resource))
  }
}

#[test]
fn used_by_local_lists_the_resources_whose_latest_version_names_it_by_name_then_id() {
  let mut local = Local::new();
  let tool = local.tool("deploy", "Deploy");
  let delivery = local.publish(("delivery", "Delivery"), &pipeline_running(tool));
  let alpha = local.publish(("alpha", "alpha flow"), &pipeline_running(tool));
  // Only the latest version counts: v1 ran the tool, v2 runs another.
  let other_tool = local.tool("lint", "Lint");
  let moved_on = local.publish(("moved", "Moved on"), &pipeline_running(tool));
  local.republish(moved_on, pipeline_running(other_tool));
  let removed = local.publish(("gone", "Gone"), &pipeline_running(tool));
  local.db.remove_resource(removed, at(100)).expect("removed");
  let own = instruction_for(InstructionScope::Pipeline { pipeline: delivery }, "Be brief");
  local.publish(("style", "Style"), &own);
  // An id in the text of an instruction names nothing.
  local.publish(
    ("decoy", "Decoy"),
    &instruction_for(InstructionScope::Project, &format!("call {tool} and {delivery}")),
  );
  let skill = Skill::new(
    "Ship".to_owned(),
    "How to ship".to_owned(),
    Vec::new(),
    InstructionScope::Step {
      pipeline: alpha,
      step: StepKey::new("check").expect("key"),
    },
  )
  .expect("skill");
  local.publish(("ship", "Ship"), &skill);

  let used = local.used_by(tool).expect("the tool's users");
  assert_eq!(
    shown(&used),
    (
      vec![(PipelineKind, "alpha flow", 1), (PipelineKind, "Delivery", 1)],
      Vec::new()
    ),
    "by name without regard to case; the tool is no user of itself, a removed pipeline and a version that no longer \
     names it are left out"
  );
  // An instruction's or a skill's scope decides nothing in a local project, so it names nothing either.
  assert_eq!(
    shown(&local.used_by(delivery).expect("the pipeline's users")).0,
    Vec::new()
  );
  assert_eq!(
    shown(&local.used_by(alpha).expect("the pipeline's users")).0,
    Vec::new()
  );
  assert_eq!(
    shown(&local.used_by(removed).expect("a removed resource answers")),
    (Vec::new(), Vec::new())
  );
}

#[test]
fn used_by_local_lists_the_runs_that_have_not_ended_by_task_number_then_run_id() {
  let mut local = Local::new();
  let tool = local.tool("deploy", "Deploy");
  let delivery = local.publish(("delivery", "Delivery"), &pipeline_running(tool));
  local.publish(("alpha", "alpha flow"), &pipeline_running(tool));
  let other_tool = local.tool("lint", "Lint");
  let other = local.publish(("other", "Other"), &pipeline_running(other_tool));
  let tasks = local.tasks(12);
  // Started in the order KON-12, KON-3, KON-3 again; a numeric order puts KON-3 first.
  local.run(tasks[11], (delivery, tool));
  let first = local.run(tasks[2], (delivery, tool));
  let second = local.run(tasks[2], (delivery, tool));
  let mut ended = local.run(tasks[4], (delivery, tool));
  local.fail(&mut ended);
  local.run(tasks[8], (other, other_tool));

  let used = local.used_by(tool).expect("the tool's users");
  assert_eq!(
    shown(&used),
    tool_users(),
    "a run pins the tool through its pipeline's step; the failed run and the other tool's run are left out"
  );
  assert_eq!(
    used.runs[..2].iter().map(|run| run.run).collect::<Vec<_>>(),
    [first.id(), second.id()],
    "the runs of one task are ordered by run id"
  );
  assert!(
    used.runs.iter().all(|run| run.holder.is_none()),
    "no one else holds a local task's claim"
  );
  let pipeline = local.used_by(delivery).expect("the pipeline's users");
  assert_eq!(pipeline.runs.len(), 3, "the pipeline itself is pinned too");
}

#[test]
fn used_by_local_refuses_a_resource_the_project_does_not_have() {
  let local = Local::new();
  assert!(matches!(
    local.used_by(ResourceId::generate()),
    Err(UsedByError::NotFound)
  ));
}

#[test]
fn used_by_local_names_an_agent_profile_by_a_step_and_the_deliver_integrator_and_not_by_an_instruction_target() {
  let mut local = Local::new();
  let profile = local.profile("builder", "Builder");
  let other = local.profile("reviewer", "Reviewer");
  let tool = local.tool("deploy", "Deploy");
  local.publish(("implement", "Implement"), &pipeline_of(agent_step(profile)));
  let integrating = pipeline_running(tool).with_deliver(Some(DeliverStep::default().with_integrator(Some(profile))));
  local.publish(("ship", "Ship"), &integrating);
  let targeting = instruction_targeting(
    AgentTargets::Profiles(BTreeSet::from([profile])),
    InstructionScope::Project,
    "Be brief",
  );
  local.publish(("brief", "Brief"), &targeting);
  // Other profiles, and every agent, are named by nothing.
  local.publish(("other-step", "Other step"), &pipeline_of(agent_step(other)));
  let elsewhere = instruction_targeting(
    AgentTargets::Profiles(BTreeSet::from([other])),
    InstructionScope::Project,
    "Be kind",
  );
  local.publish(("kind", "Kind"), &elsewhere);
  local.publish(("all", "All"), &instruction_for(InstructionScope::Project, "Be fast"));

  let used = local.used_by(profile).expect("the profile's users");
  assert_eq!(
    shown(&used).0,
    [(PipelineKind, "Implement", 1), (PipelineKind, "Ship", 1)],
    "a step's agent and the Deliver step's integrator name a profile; an instruction's targets decide nothing here"
  );
}

/// A local profile names the instructions, skills and tools it gives its agents
/// (`docs/design/team-configuration.md`, "Decided 2026-11-09").
#[test]
fn used_by_local_names_an_instruction_skill_and_tool_by_the_profiles_that_give_them() {
  let mut local = Local::new();
  let tool = local.tool("deploy", "Deploy");
  let brief = local.publish(
    ("brief", "Brief"),
    &instruction_for(InstructionScope::Project, "Be brief"),
  );
  let skill = Skill::new(
    "Ship".to_owned(),
    "How to ship".to_owned(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("skill");
  let ship = local.publish(("ship", "Ship"), &skill);
  let builder = local.profile("builder", "Builder");
  local.profile("reviewer", "Reviewer");
  let chosen = |id| orchester_core::resource::ChosenResources::new(vec![id]).expect("one");
  let giving = local
    .db
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("published")
    .body()
    .clone()
    .with_given(chosen(brief), chosen(ship), chosen(tool));
  local.republish(builder, giving);

  for resource in [brief, ship, tool] {
    assert_eq!(
      shown(&local.used_by(resource).expect("its users")).0,
      [(AgentProfileKind, "Builder", 2)],
      "only the profile that gives it, in its latest version"
    );
  }
}

#[test]
fn used_by_local_names_no_pipeline_by_the_callers_of_a_tools_access_policy() {
  let mut local = Local::new();
  let tool = local.tool("deploy", "Deploy");
  let pipeline = local.publish(("delivery", "Delivery"), &pipeline_running(tool));
  let other = local.publish(("other", "Other"), &pipeline_running(tool));
  let access = AccessPolicy::new(
    BTreeMap::from([(pipeline, AllowedSteps::All)]),
    name("Staging"),
    ApprovalMode::Never,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("access");
  let guarded = Tool::new("", ToolAction::Command(status_action()), Some(access)).expect("tool");
  local.publish(("guarded", "Guarded"), &guarded);

  for pipeline in [pipeline, other] {
    assert_eq!(
      shown(&local.used_by(pipeline).expect("the pipeline's users")).0,
      Vec::new(),
      "a tool's callers decide nothing for a local run, so they name nothing"
    );
  }
}
