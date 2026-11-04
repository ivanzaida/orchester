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
  id::{
    ClaimId, ConnectionId, ModuleId, OperationId, ProjectId, ResourceId, SecretId, ServerId, TaskId, VersionId, WorkId,
  },
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, Deliverable, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Step, StepKey,
    StepKind, Version, VersionNumber,
  },
  run::{
    BranchName, CommitId, PinnedVersion, PublishedVersions, RunSnapshot,
    bundle::{CanonicalExecutionInputs, PinnedBodies},
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
    profiles: &profiles.values().cloned().collect::<Vec<_>>(),
    instructions: &[],
    tools: &[],
    skills: &[],
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
  BundleContent::new(origin, task, limits, Deliverable::default(), fixture.inputs.clone()).expect("content")
}

fn bound(fixture: &Fixture) -> LocalBindings {
  LocalBindings {
    profiles: vec![ProfileBinding {
      resource: fixture.profile,
      connection: ConnectionId::generate(),
      account: None,
    }],
    references: vec![BoundReference::Secret {
      named: SecretId::generate(),
      local: SecretId::generate(),
    }],
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
  assert_eq!(value["format"], json!(1));
}

#[test]
fn run_inputs_refuse_another_format_unknown_fields_and_bad_limits() {
  let fixture = fixture();
  let value = serde_json::to_value(content(&fixture, team())).expect("serializes");
  for (pointer, changed, refusal) in [
    ("/format", json!(2), "format 2"),
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
