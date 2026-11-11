//! A team run whose tool step runs a team tool that sends a team secret is refused while this computer has no value
//! for that secret, which it looks up by the name the tool gives it, never by an id of the publisher's computer.

use orchester_core::{
  id::SecretId,
  resource::{FieldName, HttpAuth, HttpMethod, HttpRequest, RequestTarget},
  run::TeamPublishedVersions,
  secret::{SecretName, SecretValue},
};

use super::*;

/// The team's `sentry-issues`, an HTTP request sending `SENTRY_TOKEN`, made from its publisher's tool.
fn sentry() -> Version<TeamTool> {
  let request = HttpRequest::new(
    HttpMethod::Get,
    "https://sentry.example.test/api/issues/".into(),
    "sentry.example.test".into(),
    HttpAuth::Bearer {
      secret: SecretId::generate(),
    },
    BTreeMap::from([(
      FieldName::new("query").expect("field"),
      RequestTarget::Query("query".into()),
    )]),
    BTreeMap::new(),
  )
  .expect("request");
  let access = AccessPolicy::new(
    BTreeMap::new(),
    name("Production"),
    ApprovalMode::Never,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("access");
  let local = Tool::new("Issues.", ToolAction::Http(request), Some(access)).expect("tool");
  let team = TeamTool::from_local(&local, |_| Some(secret())).expect("team tool");
  Version::new(VersionId::generate(), VersionNumber::FIRST, team, Timestamp::now())
}

fn secret() -> SecretName {
  SecretName::new("SENTRY_TOKEN").expect("name")
}

/// The pipeline of `delivery` with a last step `fetch` that runs `tool`.
fn with_tool_step(profile: ResourceId, tool: ResourceId) -> Version<Pipeline> {
  let mut steps = delivery(profile).body().steps().to_vec();
  steps.push(Step {
    key: StepKey::new("fetch").expect("key"),
    name: name("Fetch issues"),
    kind: StepKind::Tool { tool: Some(tool) },
    instructions: PromptText::new("Fetch.").expect("prompt"),
    depends_on: BTreeSet::from([StepKey::new("review").expect("key")]),
    approval_gate: false,
    on_failure: None,
    budget: None,
  });
  Version::new(
    VersionId::generate(),
    VersionNumber::FIRST,
    Pipeline::new(steps).expect("pipeline"),
    Timestamp::now(),
  )
}

/// [`team`] whose pipeline ends with a step that runs `sentry-issues`.
fn team_with_sentry(setup: &Setup) -> Team {
  let project = project(setup);
  let root = project.repo.path();
  let (server, server_project) = link(root);
  let (profile, pipeline, tool) = (ResourceId::generate(), ResourceId::generate(), ResourceId::generate());
  let profile_version = builder(tool);
  let pipeline_version = with_tool_step(profile, tool);
  let team = sentry();
  let profiles = BTreeMap::from([(profile, profile_version.clone())]);
  let tools = BTreeMap::from([(tool, team.clone())]);
  let published = TeamPublishedVersions {
    profiles: &profiles,
    tools: &tools,
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  let snapshot = RunSnapshot::build(pipeline, &pipeline_version, published).expect("snapshot");
  let label = |slug: &str, title: &str| ResourceLabel::new(Slug::new(slug).expect("slug"), name(title));
  let labels = [
    (profile_version.id(), label("builder", "Builder")),
    (pipeline_version.id(), label("delivery", "Delivery")),
    (team.id(), label("sentry-issues", "sentry-issues")),
  ];
  let bodies = PinnedBodies {
    labels: &labels,
    profiles: &[(profile_version.id(), ExecutionProfile::of(profile_version.body()))],
    instructions: &[],
    tools: &[team],
    skills: &[],
    mcp_servers: &[],
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
    mcp: Vec::new(),
    canonical,
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn team_inputs_a_tool_step_whose_team_secret_has_no_value_here_is_refused_by_its_name() {
  let team = team_with_sentry(&Setup::new("Append one line."));
  let root = team.root();
  let (scheduler, _events) = scheduler(root);
  let head = git(root, &["rev-parse", "HEAD"]);
  let fetch = StepKey::new("fetch").expect("key");
  let not_runnable = PreflightProblem::ToolNotRunnable {
    step: fetch.clone(),
    kind: orchester_core::resource::ToolKind::Http,
  };

  let operation = team.accept(team.connection(), &head);
  assert_eq!(
    scheduler.start_accepted_run(operation).await,
    Err(Rejection::Preflight {
      problems: vec![not_runnable.clone(), PreflightProblem::SecretMissing { step: fetch }]
    })
  );

  ProjectDb::open(root)
    .expect("open")
    .set_team_secret_value(
      team.project.vault.as_ref(),
      &secret(),
      &SecretValue::new("sentry-value-5e1a".to_owned()).expect("value"),
    )
    .expect("a value here");
  let operation = team.accept(team.connection(), &head);
  assert_eq!(
    scheduler.start_accepted_run(operation).await,
    Err(Rejection::Preflight {
      problems: vec![not_runnable]
    }),
    "with a value under its name, only what no tool step runs yet is left"
  );
}
