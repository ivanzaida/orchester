//! A run's execution inputs (team authority P3): the canonical content accepted for the run, kept apart from how this
//! computer binds it and what a person here narrowed or consented to; each pinned profile bound once, consent only to
//! pinned versions, narrowing that never widens, and a content that reads back and hashes the same.

use std::{
  collections::{BTreeMap, BTreeSet},
  num::NonZeroU32,
  time::Duration,
};

use orchester_core::{
  blob::ContentHash,
  connection::{AgentConnection, AgentProvider},
  id::{
    ClaimId, ConnectionId, ModuleId, OperationId, ProjectId, ResourceId, SecretId, ServerId, TaskId, VersionId, WorkId,
  },
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, Deliverable, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey,
    StepKind, Version, VersionNumber,
  },
  run::{
    BranchName, CommitId, PinnedVersion, PublishedVersions, RunSnapshot,
    bundle::{CanonicalExecutionInputs, ExecutionProfile, PinnedBodies, ResourceLabel},
    inputs::{
      BoundReference, BundleContent, InputsError, InputsOrigin, LocalBindings, LocalConsent, ProfileBinding, RunInputs,
      TaskAtStart, TeamAcceptance,
    },
  },
  sharing::TaskRevision,
  task::{TaskLimits, identity::TeamTaskRef},
  time::Timestamp,
  tokens::TokenBudget,
};
use serde_json::json;

struct Fixture {
  profile: ResourceId,
  snapshot: RunSnapshot,
  inputs: CanonicalExecutionInputs,
}

fn version<B>(number: u32, body: B) -> Version<B> {
  let number = VersionNumber::new(NonZeroU32::new(number).expect("positive version"));
  Version::new(VersionId::generate(), number, body, Timestamp::from_unix_millis(1))
}

/// A one-step pipeline whose agent step reads the repository only, run by one profile.
fn fixture() -> Fixture {
  let profile = ResourceId::generate();
  let step = Step {
    key: StepKey::new("review").expect("key"),
    name: Name::new("Review").expect("name"),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = version(2, Pipeline::new(vec![step]).expect("pipeline"));
  let body = AgentProfile::new(
    Name::new("Reviewer").expect("role"),
    "Reviews.".into(),
    PromptText::new("Review.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("model-r").expect("model"),
    ProviderSettings::default(),
  );
  let profiles = BTreeMap::from([(profile, version(1, body))]);
  let empty = BTreeMap::new();
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &empty,
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  let snapshot = RunSnapshot::build(ResourceId::generate(), &pipeline, published).expect("snapshot");
  let bodies = PinnedBodies {
    profiles: &profiles
      .values()
      .map(|version| (version.id(), ExecutionProfile::of(version.body())))
      .collect::<Vec<_>>(),
    instructions: &[],
    tools: &[],
    skills: &[],
    mcp_servers: &[],
    labels: &[
      (pipeline.id(), label("review-flow", "Review flow")),
      (profiles[&profile].id(), label("reviewer", "Reviewer")),
    ],
  };
  let inputs = CanonicalExecutionInputs::pin(&snapshot, bodies).expect("inputs");
  Fixture {
    profile,
    snapshot,
    inputs,
  }
}

fn team() -> InputsOrigin {
  InputsOrigin::Team(TeamAcceptance {
    task: TeamTaskRef {
      server: ServerId::generate(),
      project: ProjectId::generate(),
      task: TaskId::generate(),
    },
    definition_revision: TaskRevision::try_from(7).expect("revision"),
    work: WorkId::generate(),
    claim: ClaimId::generate(),
    operation: OperationId::generate(),
    module: ModuleId::generate(),
    branch: BranchName::new("orchester/module-webhooks").expect("branch"),
    base_commit: CommitId::new(&"b".repeat(40)).expect("commit"),
    bundle: ContentHash::new(&"c".repeat(64)).expect("hash"),
  })
}

fn label(slug: &str, name: &str) -> ResourceLabel {
  ResourceLabel::new(Slug::new(slug).expect("slug"), Name::new(name).expect("name"))
}

fn content(fixture: &Fixture, origin: InputsOrigin) -> BundleContent {
  let limits = TaskLimits::new(
    Some(TokenBudget::new(50_000).expect("budget")),
    Some(Duration::from_secs(600)),
  )
  .expect("limits");
  let task = TaskAtStart {
    title: "Review the webhooks".into(),
    brief: "Check the retries.".into(),
    criteria: vec!["Retries are bounded".into()],
  };
  BundleContent::new(
    origin,
    (NonZeroU32::new(12).expect("number"), task),
    (limits, Deliverable::default()),
    fixture.inputs.clone(),
  )
  .expect("content")
}

fn bound(fixture: &Fixture) -> LocalBindings {
  LocalBindings {
    profiles: vec![ProfileBinding {
      resource: fixture.profile,
      connection: ConnectionId::generate(),
      account: None,
      required_cli: None,
      mcp_servers: Vec::new(),
    }],
    references: vec![BoundReference::Secret {
      named: SecretId::generate(),
      local: SecretId::generate(),
    }],
    team_mcp: Vec::new(),
  }
}

#[test]
fn run_inputs_read_back_and_hash_the_same() {
  let fixture = fixture();
  let content = content(&fixture, team());
  let text = serde_json::to_string(&content).expect("serializes");
  let read: BundleContent = serde_json::from_str(&text).expect("reads");
  assert_eq!(read, content);
  assert_eq!(read.sha256().expect("hash"), content.sha256().expect("hash"));
  assert_eq!(read.limits(), content.limits());
  let local = self::content(&fixture, InputsOrigin::Local);
  assert_ne!(local.sha256().expect("hash"), content.sha256().expect("hash"));
  let value = serde_json::to_value(&local).expect("serializes");
  assert_eq!(value["origin"], json!({"kind": "local"}));
  assert_eq!(value["format"], json!(2));
  assert_eq!(value["task_number"], json!(12));
}

#[test]
fn run_inputs_label_pinned_resources_from_the_canonical_inputs() {
  let fixture = fixture();
  let task = TaskAtStart {
    title: "Review".into(),
    brief: String::new(),
    criteria: Vec::new(),
  };
  let make = |task: TaskAtStart| {
    BundleContent::new(
      team(),
      (NonZeroU32::MIN, task),
      (TaskLimits::NONE, Deliverable::default()),
      fixture.inputs.clone(),
    )
  };
  let content = make(task.clone()).expect("content");
  let pipeline = content
    .inputs()
    .label(fixture.snapshot.pipeline().resource)
    .expect("the pipeline's label");
  assert_eq!(pipeline.slug().as_str(), "review-flow");
  let profile = content.inputs().label(fixture.profile).expect("the profile's label");
  assert_eq!(profile.name().as_str(), "Reviewer");
  assert!(content.inputs().label(ResourceId::generate()).is_none());
  let blank = TaskAtStart {
    title: " ".into(),
    ..task
  };
  assert!(matches!(make(blank), Err(InputsError::TaskTitle(_))));
}

#[test]
fn run_inputs_resolve_a_profile_on_its_binding_and_the_task_at_start() {
  let fixture = fixture();
  let server = ConnectionId::generate();
  let mut bindings = LocalBindings {
    references: vec![BoundReference::McpServer {
      named: ConnectionId::generate(),
      local: server,
    }],
    ..bound(&fixture)
  };
  bindings.profiles[0].mcp_servers = vec![server];
  let inputs = RunInputs::new(content(&fixture, team()), bindings.clone(), LocalConsent::default()).expect("inputs");
  let pinned = inputs.bundle().inputs().profiles()[0].pinned();
  let profile = inputs.profile(pinned).expect("the pinned profile");
  assert_eq!(profile.connection(), bindings.profiles[0].connection);
  assert_eq!(profile.role().as_str(), "Reviewer");
  assert_eq!(profile.mcp_servers().ids(), [server]);
  assert_eq!(inputs.bound_mcp_servers().ids().len(), 1);
  let other = PinnedVersion {
    version: VersionId::generate(),
    ..pinned
  };
  assert!(inputs.profile(other).is_none());
  assert!(inputs.is_team());

  let key = orchester_core::project::ProjectKey::new("WEB").expect("key");
  let task = inputs.task(TaskId::generate(), &key, Timestamp::from_unix_millis(5));
  assert_eq!(task.key().to_string(), "WEB-12");
  assert_eq!(task.details().title.as_str(), "Review the webhooks");
  assert_eq!(task.details().pipeline, Some(fixture.snapshot.pipeline().resource));
  assert_eq!(task.pipeline_version(), Some(fixture.snapshot.pipeline().number));
  assert_eq!(task.revision(), TaskRevision::try_from(7).expect("revision"));
  assert!(task.module().is_some());
  assert_eq!(task.organization().delivers(), Some(Deliverable::default()));
}

#[test]
fn run_inputs_refuse_another_format_unknown_fields_and_bad_limits() {
  let fixture = fixture();
  let value = serde_json::to_value(content(&fixture, team())).expect("serializes");
  for (pointer, changed, refusal) in [
    ("/format", json!(1), "format 1"),
    ("/max_duration_ms", json!(0), "greater than zero"),
  ] {
    let mut changed_value = value.clone();
    *changed_value.pointer_mut(pointer).expect("field") = changed;
    let error = serde_json::from_value::<BundleContent>(changed_value).expect_err("refused");
    assert!(error.to_string().contains(refusal), "{pointer}: {error}");
  }
  let mut unknown = value;
  unknown["origin"]["latest"] = json!(true);
  assert!(serde_json::from_value::<BundleContent>(unknown).is_err());
}

#[test]
fn run_inputs_bind_each_pinned_profile_once() {
  let fixture = fixture();
  let content = content(&fixture, team());
  let inputs = RunInputs::new(content.clone(), bound(&fixture), LocalConsent::default()).expect("inputs");
  assert!(inputs.pinned_by(&fixture.snapshot));
  assert!(!inputs.pinned_by(&self::fixture().snapshot), "another pipeline's run");
  let missing = RunInputs::new(content.clone(), LocalBindings::default(), LocalConsent::default());
  assert_eq!(missing, Err(InputsError::MissingBinding(fixture.profile)));
  let mut twice = bound(&fixture);
  twice.profiles.push(twice.profiles[0].clone());
  let twice = RunInputs::new(content.clone(), twice, LocalConsent::default());
  assert_eq!(twice, Err(InputsError::UnexpectedBinding(fixture.profile)));
  let mut other = bound(&fixture);
  let stranger = ResourceId::generate();
  other.profiles[0].resource = stranger;
  let other = RunInputs::new(content, other, LocalConsent::default());
  assert_eq!(other, Err(InputsError::UnexpectedBinding(stranger)));
}

#[test]
fn run_inputs_consent_only_to_pinned_versions_and_never_widen() {
  let fixture = fixture();
  let content = content(&fixture, team());
  let resolved = content.inputs().step_permissions();
  let pinned = content.inputs().profiles()[0].pinned();
  let consent = LocalConsent {
    accepted: vec![pinned],
    narrowed: resolved.clone(),
  };
  assert!(RunInputs::new(content.clone(), bound(&fixture), consent).is_ok());

  let unpinned = PinnedVersion {
    version: VersionId::generate(),
    ..pinned
  };
  let accepted = LocalConsent {
    accepted: vec![unpinned],
    narrowed: Vec::new(),
  };
  assert!(matches!(
    RunInputs::new(content.clone(), bound(&fixture), accepted),
    Err(InputsError::AcceptedUnpinned { .. })
  ));

  let mut wider = resolved[0].clone();
  wider.permissions.set_repository(RepositoryAccess::Write);
  let widened = LocalConsent {
    accepted: Vec::new(),
    narrowed: vec![wider],
  };
  assert_eq!(
    RunInputs::new(content.clone(), bound(&fixture), widened),
    Err(InputsError::NarrowedWider(resolved[0].step.clone()))
  );
  let mut unknown = resolved[0].clone();
  unknown.step = StepKey::new("deploy").expect("key");
  let unknown_step = LocalConsent {
    accepted: Vec::new(),
    narrowed: vec![unknown.clone()],
  };
  assert_eq!(
    RunInputs::new(content, bound(&fixture), unknown_step),
    Err(InputsError::NarrowedUnknown(unknown.step))
  );
}

#[test]
fn run_inputs_keep_local_bindings_out_of_the_canonical_content() {
  let fixture = fixture();
  let bindings = bound(&fixture);
  let content = content(&fixture, team());
  let text = serde_json::to_string(&content).expect("serializes");
  let connection = bindings.profiles[0].connection.to_string();
  assert!(
    !text.contains(&connection),
    "a local connection is not canonical content"
  );
  let BoundReference::Secret { named, local } = &bindings.references[0] else {
    panic!("a secret reference");
  };
  let value = serde_json::to_value(&bindings).expect("serializes");
  assert_eq!(
    value["references"][0],
    json!({"kind": "secret", "named": named, "local": local})
  );
  let inputs = RunInputs::new(content.clone(), bindings, LocalConsent::default()).expect("inputs");
  assert_eq!(inputs.bundle(), &content);
}

#[test]
fn run_inputs_connection_kind_mismatch_is_an_unbound_profile() {
  let fixture = fixture();
  let mut bindings = bound(&fixture);
  bindings.profiles[0].required_cli = Some(AgentProvider::CodexCli);
  let local = bindings.profiles[0].connection;
  let inputs = RunInputs::new(content(&fixture, team()), bindings, LocalConsent::default()).expect("inputs");
  for (provider, matches) in [(AgentProvider::CodexCli, true), (AgentProvider::ClaudeCodeCli, false)] {
    let connection = AgentConnection::new(
      local,
      Name::new("Own account").expect("name"),
      provider,
      std::env::current_dir().expect("directory").join("agent"),
      Timestamp::now(),
    )
    .expect("absolute path")
    .into_connection();
    assert_eq!(
      inputs.check_connection(fixture.profile, &connection),
      if matches {
        Ok(())
      } else {
        Err(InputsError::MissingBinding(fixture.profile))
      }
    );
  }
}

#[test]
fn run_inputs_connection_kind_absent_in_older_bindings_and_local_runs_stay_usable() {
  let fixture = fixture();
  let bindings = bound(&fixture);
  let value = serde_json::to_value(&bindings).expect("json");
  assert!(value["profiles"][0].get("required_cli").is_none(), "older JSON shape");
  let mut older: LocalBindings = serde_json::from_value(value).expect("older bindings read");
  let connection = AgentConnection::new(
    older.profiles[0].connection,
    Name::new("Own account").expect("name"),
    AgentProvider::ClaudeCodeCli,
    std::env::current_dir().expect("directory").join("agent"),
    Timestamp::now(),
  )
  .expect("absolute path")
  .into_connection();
  let inputs = RunInputs::new(content(&fixture, team()), older.clone(), LocalConsent::default()).expect("inputs");
  assert_eq!(inputs.check_connection(fixture.profile, &connection), Ok(()));
  older.profiles[0].required_cli = Some(AgentProvider::CodexCli);
  let local =
    RunInputs::new(content(&fixture, InputsOrigin::Local), older, LocalConsent::default()).expect("local inputs");
  assert_eq!(local.check_connection(fixture.profile, &connection), Ok(()));
}
