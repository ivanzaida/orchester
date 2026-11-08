//! Team runs started from the inputs the team server accepted (team authority P3b): the runner starts one from the
//! inputs the desktop kept by their operation, with no definition of its team task or of the team's resources on this
//! computer, executes it, and goes on after a restart from the same inputs; a publication here after the start changes
//! nothing for it; what this computer cannot run is refused before a run is recorded; and a local project's run is as
//! it was. Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test team_inputs team_inputs`.

#[path = "team_inputs/connection_kinds.rs"]
mod connection_kinds;
#[path = "team_inputs/mcp.rs"]
mod mcp;
mod support;

use std::{
  collections::{BTreeMap, BTreeSet},
  num::NonZeroU32,
};

use orchester_core::{
  blob::ContentHash,
  connection::REASONING_EFFORT,
  id::{
    ClaimId, ConnectionId, IdempotencyKey, ModuleId, OperationId, ProjectId, ResourceId, RunId, ServerId, TaskId,
    VersionId, WorkId,
  },
  name::Name,
  orchester_tools::ProjectTool,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, StepPermissions},
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, AllowedSteps, ApprovalMode, CommandAction, CommandArg, CommandProgram,
    Deliverable, DeniedBehavior, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step,
    StepKey, StepKind, TeamAgentProfile, TeamMcpServer, Tool, ToolAction, Version, VersionNumber,
  },
  run::{
    BranchName, CommitId, PauseReason, PreflightProblem, PublishedVersions, RunSnapshot, RunStatus, StepStatus,
    bundle::{CanonicalExecutionInputs, ExecutionProfile, PinnedBodies, ResourceLabel},
    inputs::{
      BundleContent, InputsOrigin, LocalBindings, LocalConsent, ProfileBinding, RunInputs, TaskAtStart, TeamAcceptance,
      TeamMcpBinding, team_mcp_id,
    },
  },
  sharing::TaskRevision,
  store::{ConnectionStore, ResourceStore, RunStore, TaskStore},
  task::{TaskLimits, identity::TeamTaskRef},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use self::support::{PATIENCE, Setup, TestProject, git, project, reported, scheduler, wait_for, wait_for_idle};

fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}

/// A team project: the test project linked to a server project, and the team task the server accepted a run of.
struct Team {
  project: TestProject,
  task: TaskId,
  team_task: TeamTaskRef,
  profile: ResourceId,
  pipeline: ResourceId,
  tool: ResourceId,
  /// The team MCP servers the profile names and the inputs pin, by server resource id.
  mcp: Vec<ResourceId>,
  canonical: CanonicalExecutionInputs,
}

/// Links the project at `root` to a new server project, as a member; the server and the project.
fn link(root: &std::path::Path) -> (ServerId, ProjectId) {
  let (server, server_project) = (ServerId::generate(), ProjectId::generate());
  rusqlite::Connection::open(root.join(".orchester").join("orchester.db"))
    .expect("open")
    .execute(
      "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor, \
       last_synced_at) VALUES (1, ?1, ?2, 'ORC', 'member', 1, 0, 1)",
      [server.to_string(), server_project.to_string()],
    )
    .expect("link the project");
  (server, server_project)
}

/// The team's Builder profile, version 1, as the server published it: on the publisher's connection.
fn builder() -> Version<AgentProfile> {
  let settings =
    ProviderSettings::new(BTreeMap::from([(REASONING_EFFORT.to_owned(), "low".to_owned())])).expect("settings");
  let body = AgentProfile::new(
    name("Builder"),
    "Implements tasks.".into(),
    PromptText::new("Work carefully.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    settings,
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  ));
  Version::new(VersionId::generate(), VersionNumber::FIRST, body, Timestamp::now())
}

/// The team's pipeline, version 1: Implement, then Review, both run by `profile`.
fn delivery(profile: ResourceId) -> Version<Pipeline> {
  let step = |key: &str, title: &str, after: &[&str]| Step {
    key: StepKey::new(key).expect("key"),
    name: name(title),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Change README.md for {{task.id}}.").expect("prompt"),
    depends_on: after
      .iter()
      .map(|key| StepKey::new(key).expect("key"))
      .collect::<BTreeSet<_>>(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let steps = vec![
    step("implement", "Implement", &[]),
    step("review", "Review", &["implement"]),
  ];
  Version::new(
    VersionId::generate(),
    VersionNumber::FIRST,
    Pipeline::new(steps).expect("pipeline"),
    Timestamp::now(),
  )
}

/// The team's check, version 1: a command tool its access rules offer to the pipeline's Implement step only.
fn check(pipeline: ResourceId) -> Version<Tool> {
  let action = CommandAction::new(
    CommandProgram::new("git").expect("program"),
    vec![CommandArg::new("status").expect("argument")],
  );
  let access = AccessPolicy::new(
    BTreeMap::from([(
      pipeline,
      AllowedSteps::Only(BTreeSet::from([StepKey::new("implement").expect("key")])),
    )]),
    name("Worktree"),
    ApprovalMode::OncePerRun,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("access");
  let body = Tool::new("Shows the worktree.", ToolAction::Command(action), Some(access)).expect("tool");
  Version::new(VersionId::generate(), VersionNumber::FIRST, body, Timestamp::now())
}

/// The team's Builder profile, its two-step pipeline (Implement, then Review) and a check, published on the server
/// only: no resource row of any is on this computer.
fn team(setup: &Setup) -> Team {
  team_pinning(setup, Vec::new())
}

/// [`team`] whose Builder profile names `servers`, each pinned at version 1 under a server id of its own.
fn team_pinning(setup: &Setup, servers: Vec<TeamMcpServer>) -> Team {
  let project = project(setup);
  let root = project.repo.path();
  let (server, server_project) = link(root);
  let (profile, pipeline, tool) = (ResourceId::generate(), ResourceId::generate(), ResourceId::generate());
  let profile_version = builder();
  let pipeline_version = delivery(profile);
  let tool_version = check(pipeline);
  let profiles = BTreeMap::from([(profile, profile_version.clone())]);
  let tools = BTreeMap::from([(tool, tool_version.clone())]);
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &tools,
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  let snapshot = RunSnapshot::build(pipeline, &pipeline_version, published).expect("snapshot");
  let label = |slug: &str, title: &str| ResourceLabel::new(Slug::new(slug).expect("slug"), name(title));
  let mcp: Vec<ResourceId> = servers.iter().map(|_| ResourceId::generate()).collect();
  let pinned_servers: Vec<(ResourceId, Version<TeamMcpServer>)> = mcp
    .iter()
    .zip(servers)
    .map(|(id, server)| {
      let version = Version::new(VersionId::generate(), VersionNumber::FIRST, server, Timestamp::now());
      (*id, version)
    })
    .collect();
  let execution_profile = if mcp.is_empty() {
    ExecutionProfile::of(profile_version.body())
  } else {
    let body = profile_version.body();
    let team = TeamAgentProfile::new(
      body.role().clone(),
      body.responsibilities().to_owned(),
      body.instructions().clone(),
      (setup.provider, body.model().clone(), body.settings().clone()),
    )
    .with_mcp_servers(mcp.clone())
    .expect("distinct servers");
    ExecutionProfile::of_team(&team)
  };
  let mut labels = vec![
    (profile_version.id(), label("builder", "Builder")),
    (pipeline_version.id(), label("delivery", "Delivery")),
    (tool_version.id(), label("worktree-status", "Worktree status")),
  ];
  labels.extend(
    pinned_servers
      .iter()
      .enumerate()
      .map(|(at, (_, version))| (version.id(), label(&format!("server-{at}"), &format!("Server {at}")))),
  );
  let bodies = PinnedBodies {
    labels: &labels,
    profiles: &[(profile_version.id(), execution_profile)],
    instructions: &[],
    tools: &[tool_version],
    skills: &[],
    mcp_servers: &pinned_servers,
  };
  let canonical = CanonicalExecutionInputs::pin(&snapshot, bodies).expect("inputs");
  let team_task = TeamTaskRef {
    server,
    project: server_project,
    task: TaskId::generate(),
  };
  let task = ProjectDb::open(root)
    .expect("open")
    .record_team_task(team_task, revision())
    .expect("identity");
  Team {
    project,
    task,
    team_task,
    profile,
    pipeline,
    tool,
    mcp,
    canonical,
  }
}

fn revision() -> TaskRevision {
  TaskRevision::try_from(3).expect("revision")
}

impl Team {
  fn root(&self) -> &std::path::Path {
    self.project.repo.path()
  }

  /// Keeps the inputs the server accepted for a run of the team task from `base`, its profile bound to `connection`;
  /// the operation that accepted them.
  fn accept(&self, connection: ConnectionId, base: &str) -> OperationId {
    self.accept_with_cli(connection, base, None)
  }

  fn accept_with_cli(
    &self,
    connection: ConnectionId,
    base: &str,
    required_cli: Option<orchester_core::connection::AgentProvider>,
  ) -> OperationId {
    self.accept_bound(connection, base, (required_cli, Vec::new()))
  }

  /// [`Self::accept`] with the program path this computer set for each team MCP server of `team_mcp` (the servers the
  /// profile names, in order).
  fn accept_bound(
    &self,
    connection: ConnectionId,
    base: &str,
    (required_cli, team_mcp): (Option<orchester_core::connection::AgentProvider>, Vec<TeamMcpBinding>),
  ) -> OperationId {
    let operation = OperationId::generate();
    let origin = InputsOrigin::Team(TeamAcceptance {
      task: self.team_task,
      definition_revision: revision(),
      work: WorkId::generate(),
      claim: ClaimId::generate(),
      operation,
      module: ModuleId::generate(),
      branch: BranchName::new("main").expect("branch"),
      base_commit: CommitId::new(base).expect("commit"),
      bundle: ContentHash::new(&"c".repeat(64)).expect("hash"),
    });
    let task = TaskAtStart {
      title: "Say hello".into(),
      brief: "Work slowly.".into(),
      criteria: vec!["README.md ends with hello".into()],
    };
    let content = BundleContent::new(
      origin,
      (NonZeroU32::new(7).expect("number"), task),
      (TaskLimits::NONE, Deliverable::default()),
      self.canonical.clone(),
    )
    .expect("content");
    let bindings = LocalBindings {
      profiles: vec![ProfileBinding {
        resource: self.profile,
        connection,
        account: None,
        required_cli,
        mcp_servers: team_mcp.iter().map(|bound| team_mcp_id(bound.server)).collect(),
      }],
      team_mcp,
    };
    let inputs = RunInputs::new(content, bindings, LocalConsent::default()).expect("inputs");
    ProjectDb::open(self.root())
      .expect("open")
      .accept_team_inputs(self.task, &inputs, Timestamp::now())
      .expect("kept");
    operation
  }

  /// The project's agent connection.
  fn connection(&self) -> ConnectionId {
    ProjectDb::open(self.root())
      .expect("open")
      .connections()
      .expect("connections")[0]
      .id()
  }
}

/// A publication arriving here after the run started, of the team profile's pinned version with another role (the
/// project's own Builder version, renamed), as a replica of the team's library would keep it: what a run that read
/// the published-resource tables would now give its agents. A member makes no publication, so it is written as rows.
fn publish_changed_profile(root: &std::path::Path, team_profile: ResourceId) {
  let raw = rusqlite::Connection::open(root.join(".orchester").join("orchester.db")).expect("open");
  raw
    .execute(
      "INSERT INTO resources (id, kind, slug, name, created_at) VALUES (?1, 'agent_profile', 'team-builder', \
       'Changed', 1)",
      [team_profile.to_string()],
    )
    .expect("a resource row");
  let inserted = raw
    .execute(
      "INSERT INTO resource_versions (id, resource_id, number, body, published_at) \
       SELECT ?1, ?2, 1, json_set(body, '$.body.role', 'Changed'), published_at FROM resource_versions \
       WHERE resource_id = (SELECT id FROM resources WHERE kind = 'agent_profile' AND slug = 'builder')",
      [VersionId::generate().to_string(), team_profile.to_string()],
    )
    .expect("a version row");
  assert_eq!(inserted, 1);
  let changed = ProjectDb::open(root)
    .expect("open")
    .version::<AgentProfile>(team_profile, VersionNumber::FIRST)
    .expect("read")
    .expect("the changed version");
  assert_eq!(changed.body().role().as_str(), "Changed", "the replica row reads back");
}

fn step_status(run: &orchester_core::run::Run, key: &str) -> Option<StepStatus> {
  let key = StepKey::new(key).expect("key");
  run
    .current_attempt()?
    .executions()
    .iter()
    .find(|execution| execution.step() == &key)
    .map(|execution| execution.status())
}

/// Starts the run of `operation`, waits for its first step to pass and pauses the run there.
async fn start_and_pause(team: &Team, operation: OperationId) -> RunId {
  let root = team.root();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler.start_accepted_run(operation).await.expect("the run starts");
  support::wait_for_turns(root, &mut events, id, 1).await;
  tokio::time::timeout(PATIENCE, scheduler.pause_run(id))
    .await
    .expect("answered in time")
    .expect("the pause is requested");
  let paused = wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::Requested));
  assert_eq!(step_status(&paused, "implement"), Some(StepStatus::Passed));
  assert_eq!(step_status(&paused, "review"), Some(StepStatus::Pending));
  assert_eq!(
    scheduler.start_accepted_run(operation).await,
    Ok(id),
    "the operation has one run"
  );
  wait_for_idle(&scheduler).await;
  id
}

/// The checks the paused run `id` offers and the connection its steps continue on after a plan limit, read from its
/// inputs alone: the pinned check by its accepted slug for Implement only, and the profile's bound connection.
fn assert_resolved_from_inputs(team: &Team, id: RunId) {
  let db = ProjectDb::open(team.root()).expect("open");
  assert_eq!(db.resource(team.tool).expect("read"), None, "no team tool row");
  let run = db.run(id).expect("read").expect("the run");
  let implement = StepKey::new("implement").expect("key");
  let checks = db.run_checks(&run, &implement).expect("the checks");
  let names: Vec<&str> = checks.iter().map(ProjectTool::name).collect();
  assert_eq!(names, ["worktree-status"], "the accepted check by its accepted slug");
  assert_eq!(checks[0].approval(), ApprovalMode::OncePerRun);
  let review = StepKey::new("review").expect("key");
  assert!(db.run_checks(&run, &review).expect("the checks").is_empty());
  for step in [&implement, &review] {
    let connection = db
      .step_agent_connection(&run, step)
      .expect("read")
      .expect("the bound connection");
    assert_eq!(connection.id(), team.connection(), "the profile's bound connection");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn team_inputs_a_team_run_executes_and_restarts_without_team_task_or_resource_rows() {
  let team = team(&Setup::new("Work slowly."));
  let root = team.root();
  let head = git(root, &["rev-parse", "HEAD"]);
  let operation = team.accept(team.connection(), &head);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.task(team.task).expect("read"), None, "no team task definition");
  assert_eq!(db.resource(team.profile).expect("read"), None, "no team profile row");
  assert_eq!(db.resource(team.pipeline).expect("read"), None, "no team pipeline row");

  let id = start_and_pause(&team, operation).await;
  assert_resolved_from_inputs(&team, id);
  publish_changed_profile(root, team.profile);
  assert_resolved_from_inputs(&team, id);

  // A runner started again goes on from the run's inputs.
  let (scheduler, mut events) = scheduler(root);
  scheduler.resume_run(id).await.expect("the run resumes");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  wait_for_idle(&scheduler).await;
  assert_eq!(done.task(), team.task);
  assert_eq!(done.branch().as_str(), "orchester/orc-7-run-1");
  assert_eq!(done.base().as_str(), head);
  assert_eq!(done.base_branch().map(BranchName::as_str), Some("main"));
  assert!(done.last_checkpoint().is_some(), "the steps' work was committed");

  let db = ProjectDb::open(root).expect("open");
  let roles: Vec<String> = reported(&db, id, "prompt: ");
  assert_eq!(
    roles,
    ["# Your role: Builder", "# Your role: Builder"],
    "both steps run the accepted profile"
  );
  assert_eq!(
    db.task(team.task).expect("read"),
    None,
    "the runner kept no team task definition"
  );
  assert!(db.run_inputs(id).expect("read").is_some());
  assert_eq!(db.runs_of_task(team.task).expect("runs").len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn team_inputs_what_this_computer_cannot_run_is_refused_before_a_run() {
  let team = team(&Setup::new("Append one line."));
  let root = team.root();
  let (scheduler, _events) = scheduler(root);
  assert_eq!(
    scheduler.start_accepted_run(OperationId::generate()).await,
    Err(Rejection::InputsNotAccepted)
  );
  let missing = "f".repeat(40);
  let operation = team.accept(ConnectionId::generate(), &missing);
  let refused = scheduler.start_accepted_run(operation).await;
  let missing = orchester_core::revision::Revision::new(&missing).expect("revision");
  assert_eq!(
    refused,
    Err(Rejection::Preflight {
      problems: vec![
        PreflightProblem::ConnectionMissing {
          step: StepKey::new("implement").expect("key"),
        },
        PreflightProblem::ConnectionMissing {
          step: StepKey::new("review").expect("key"),
        },
        PreflightProblem::BaseCommitMissing { commit: missing },
      ]
    })
  );
  let db = ProjectDb::open(root).expect("open");
  assert!(db.runs_of_task(team.task).expect("runs").is_empty());
  assert_eq!(db.run_of_operation(operation).expect("read"), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn team_inputs_a_local_run_is_unchanged_by_a_publication_after_its_start() {
  let project = project(&Setup {
    review: true,
    ..Setup::new("Work slowly.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  support::wait_for_turns(root, &mut events, id, 1).await;
  scheduler.pause_run(id).await.expect("the pause is requested");
  wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;

  let mut db = ProjectDb::open(root).expect("open");
  let builder = db
    .resources(orchester_core::resource::ResourceKind::AgentProfile)
    .expect("profiles")[0]
    .id();
  let v1 = db
    .version::<AgentProfile>(builder, VersionNumber::FIRST)
    .expect("read")
    .expect("v1");
  let mut draft = v1.to_draft(Timestamp::now());
  let changed = AgentProfile::new(
    name("Changed"),
    String::new(),
    PromptText::new("Work otherwise.").expect("prompt"),
    v1.body().connection(),
    v1.body().model().clone(),
    v1.body().settings().clone(),
  )
  .with_permissions(v1.body().permissions().clone());
  draft.edit(changed, Timestamp::now());
  db.save_draft(builder, &draft).expect("save the draft");
  db.publish_draft::<AgentProfile>(builder, Timestamp::now())
    .expect("publish v2");

  scheduler.resume_run(id).await.expect("the run resumes");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  wait_for_idle(&scheduler).await;
  let roles = reported(&db, id, "prompt: ");
  assert_eq!(
    roles,
    ["# Your role: Builder", "# Your role: Builder"],
    "the run keeps its pinned v1"
  );
  assert_eq!(
    db.run_inputs(id).expect("read"),
    None,
    "a local run is recorded as before"
  );
}
