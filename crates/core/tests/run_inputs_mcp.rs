//! A run's profiles each get only the MCP servers their own binding names (team authority §7), not every server the
//! run binds.

use std::{
  collections::{BTreeMap, BTreeSet},
  num::NonZeroU32,
};

use orchester_core::{
  blob::ContentHash,
  id::{ClaimId, ConnectionId, ModuleId, OperationId, ProjectId, ResourceId, ServerId, TaskId, VersionId, WorkId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, Deliverable, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey,
    StepKind, Version, VersionNumber,
  },
  run::{
    BranchName, CommitId, PublishedVersions, RunSnapshot,
    bundle::{CanonicalExecutionInputs, PinnedBodies, ResourceLabel},
    inputs::{
      BoundReference, BundleContent, InputsError, InputsOrigin, LocalBindings, LocalConsent, ProfileBinding, RunInputs,
      TaskAtStart, TeamAcceptance,
    },
  },
  sharing::TaskRevision,
  task::{TaskLimits, identity::TeamTaskRef},
  time::Timestamp,
};

fn version<B>(number: u32, body: B) -> Version<B> {
  let number = VersionNumber::new(NonZeroU32::new(number).expect("positive version"));
  Version::new(VersionId::generate(), number, body, Timestamp::from_unix_millis(1))
}

fn step(key: &str, profile: ResourceId) -> Step {
  Step {
    key: StepKey::new(key).expect("key"),
    name: Name::new(key).expect("name"),
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
  }
}

fn profile_body(role: &str) -> AgentProfile {
  AgentProfile::new(
    Name::new(role).expect("role"),
    "Works.".into(),
    PromptText::new("Work.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("model-r").expect("model"),
    ProviderSettings::default(),
  )
}

fn label(slug: &str) -> ResourceLabel {
  ResourceLabel::new(Slug::new(slug).expect("slug"), Name::new(slug).expect("name"))
}

/// Inputs of a two-step pipeline run by two profiles, with the profiles' ids.
fn two_profiles() -> (CanonicalExecutionInputs, [ResourceId; 2]) {
  let ids = [ResourceId::generate(), ResourceId::generate()];
  let pipeline = version(
    1,
    Pipeline::new(vec![step("first", ids[0]), step("second", ids[1])]).expect("pipeline"),
  );
  let profiles = BTreeMap::from([
    (ids[0], version(1, profile_body("first"))),
    (ids[1], version(1, profile_body("second"))),
  ]);
  let empty = BTreeMap::new();
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &empty,
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  let snapshot = RunSnapshot::build(ResourceId::generate(), &pipeline, published).expect("snapshot");
  let bodies = PinnedBodies {
    profiles: &profiles.values().cloned().collect::<Vec<_>>(),
    instructions: &[],
    tools: &[],
    skills: &[],
    labels: &[
      (pipeline.id(), label("flow")),
      (profiles[&ids[0]].id(), label("first")),
      (profiles[&ids[1]].id(), label("second")),
    ],
  };
  (CanonicalExecutionInputs::pin(&snapshot, bodies).expect("inputs"), ids)
}

fn binding(resource: ResourceId, mcp_servers: Vec<ConnectionId>) -> ProfileBinding {
  ProfileBinding {
    resource,
    connection: ConnectionId::generate(),
    account: None,
    required_cli: None,
    mcp_servers,
  }
}

fn reference(local: ConnectionId) -> BoundReference {
  BoundReference::McpServer {
    named: ConnectionId::generate(),
    local,
  }
}

fn make(canonical: &CanonicalExecutionInputs, bindings: LocalBindings) -> Result<RunInputs, InputsError> {
  let origin = InputsOrigin::Team(TeamAcceptance {
    task: TeamTaskRef {
      server: ServerId::generate(),
      project: ProjectId::generate(),
      task: TaskId::generate(),
    },
    definition_revision: TaskRevision::try_from(1).expect("revision"),
    work: WorkId::generate(),
    claim: ClaimId::generate(),
    operation: OperationId::generate(),
    module: ModuleId::generate(),
    branch: BranchName::new("orchester/module-x").expect("branch"),
    base_commit: CommitId::new(&"b".repeat(40)).expect("commit"),
    bundle: ContentHash::new(&"c".repeat(64)).expect("hash"),
  });
  let task = TaskAtStart {
    title: "Work".into(),
    brief: String::new(),
    criteria: Vec::new(),
  };
  let content = BundleContent::new(
    origin,
    (NonZeroU32::MIN, task),
    (TaskLimits::NONE, Deliverable::default()),
    canonical.clone(),
  )
  .expect("content");
  RunInputs::new(content, bindings, LocalConsent::default())
}

/// The MCP servers the agents of the pinned profile `resource` get.
fn servers_of(inputs: &RunInputs, resource: ResourceId) -> Vec<ConnectionId> {
  let pinned = inputs
    .bundle()
    .inputs()
    .profiles()
    .iter()
    .map(|body| body.pinned())
    .find(|pinned| pinned.resource == resource)
    .expect("pinned profile");
  inputs
    .profile(pinned)
    .expect("resolved profile")
    .mcp_servers()
    .ids()
    .to_vec()
}

#[test]
fn each_profile_gets_only_the_mcp_servers_it_names() {
  let (canonical, ids) = two_profiles();
  let (docs, issues, shared) = (
    ConnectionId::generate(),
    ConnectionId::generate(),
    ConnectionId::generate(),
  );
  let bindings = LocalBindings {
    profiles: vec![
      binding(ids[0], vec![docs, shared]),
      binding(ids[1], vec![issues, shared]),
    ],
    references: vec![reference(docs), reference(issues), reference(shared)],
  };
  let inputs = make(&canonical, bindings).expect("inputs");
  assert_eq!(servers_of(&inputs, ids[0]), [docs, shared]);
  assert_eq!(servers_of(&inputs, ids[1]), [issues, shared]);
  assert_eq!(inputs.bound_mcp_servers().ids(), [docs, issues, shared]);
}

#[test]
fn a_profile_naming_no_server_gets_none_even_when_others_are_bound() {
  let (canonical, ids) = two_profiles();
  let docs = ConnectionId::generate();
  let bindings = LocalBindings {
    profiles: vec![binding(ids[0], vec![docs]), binding(ids[1], Vec::new())],
    references: vec![reference(docs)],
  };
  let inputs = make(&canonical, bindings).expect("inputs");
  assert_eq!(servers_of(&inputs, ids[0]), [docs]);
  assert!(servers_of(&inputs, ids[1]).is_empty());
}

#[test]
fn a_profile_cannot_name_a_server_the_inputs_do_not_bind() {
  let (canonical, ids) = two_profiles();
  let stray = ConnectionId::generate();
  let bindings = LocalBindings {
    profiles: vec![binding(ids[0], vec![stray]), binding(ids[1], Vec::new())],
    references: Vec::new(),
  };
  assert_eq!(
    make(&canonical, bindings).expect_err("refused"),
    InputsError::UnboundMcpServer {
      profile: ids[0],
      server: stray
    }
  );
}
