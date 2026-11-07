//! The execution bundle's team MCP servers (ORC-880, schema 3): the versions its pinned profiles name, by server
//! resource id, listed with the bundle's other pinned resources, with names only, and bound into its hash. A bundle of
//! schema 2 is refused, as the earlier bumps were.

use std::{
  collections::{BTreeMap, BTreeSet},
  num::NonZeroU32,
};

use orchester_api::{
  SharedLimits,
  v2::{
    BundleStart, EXECUTION_BUNDLE_SCHEMA, ExecutionBundle, ExecutionBundleError, ResourceLabel, Subject, TaskAtStart,
  },
};
use orchester_core::{
  connection::AgentProvider,
  id::{ClaimId, ConnectionId, ModuleId, ProjectId, ResourceId, TaskId, VersionId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, Deliverable, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey,
    StepKind, TeamAgentProfile, TeamMcpServer, Version, VersionNumber,
  },
  run::{
    BranchName, CommitId, PublishedVersions, RunSnapshot,
    bundle::{CanonicalExecutionInputs, ExecutionProfile, PinnedBodies},
  },
  sharing::TaskRevision,
  time::Timestamp,
};
use serde_json::{Value, json};

fn version<B>(number: u32, body: B) -> Version<B> {
  let number = VersionNumber::new(NonZeroU32::new(number).expect("positive version"));
  Version::new(VersionId::generate(), number, body, Timestamp::from_unix_millis(1))
}

fn label(slug: &str) -> ResourceLabel {
  ResourceLabel::new(Slug::new(slug).expect("slug"), Name::new(slug).expect("name"))
}

fn server(name: &str, extra_arg: &str) -> TeamMcpServer {
  serde_json::from_value(json!({
    "name": name,
    "launch": {"kind": "program", "program": "npx", "args": ["-y", extra_arg], "variables": ["TOKEN"]},
    "exempt_tools": ["read"]
  }))
  .expect("a team MCP server")
}

fn start() -> BundleStart {
  BundleStart {
    project_id: ProjectId::generate(),
    subject: Subject::Task { id: TaskId::generate() },
    claim_id: ClaimId::generate(),
    module_id: ModuleId::generate(),
    branch: BranchName::new("module/sign-in").expect("branch"),
    base_commit: CommitId::new("0123456789abcdef0123456789abcdef01234567").expect("commit"),
    definition_revision: TaskRevision::try_from(1).expect("revision"),
    task_number: NonZeroU32::new(7).expect("a task number"),
    task: TaskAtStart {
      title: "Sign in".into(),
      brief: "Sign in.".into(),
      criteria: Vec::new(),
    },
    limits: SharedLimits {
      budget_tokens: None,
      max_duration_ms: None,
    },
    delivery: Deliverable::Changes,
  }
}

struct Fixture {
  profile: ResourceId,
  servers: [ResourceId; 2],
  bundle: ExecutionBundle,
}

/// One agent step run by a team profile that names two team MCP servers, `pencil` (v3) and `issues` (v1).
fn fixture(second_arg: &str) -> Fixture {
  let (pipeline, profile) = (ResourceId::generate(), ResourceId::generate());
  let mut servers = [ResourceId::generate(), ResourceId::generate()];
  servers.sort();
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: Name::new("implement").expect("name"),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline_version = version(1, Pipeline::new(vec![step]).expect("pipeline"));
  let model = ModelId::new("model-b").expect("model");
  let local = AgentProfile::new(
    Name::new("Builder").expect("role"),
    "Builds.".into(),
    PromptText::new("Work.").expect("prompt"),
    ConnectionId::generate(),
    model.clone(),
    ProviderSettings::default(),
  );
  let profiles = BTreeMap::from([(profile, version(2, local))]);
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &BTreeMap::new(),
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  let snapshot = RunSnapshot::build(pipeline, &pipeline_version, published).expect("snapshot");
  let team = TeamAgentProfile::new(
    Name::new("Builder").expect("role"),
    "Builds.".into(),
    PromptText::new("Work.").expect("prompt"),
    (AgentProvider::ClaudeCodeCli, model, ProviderSettings::default()),
  )
  .with_mcp_servers(servers.to_vec())
  .expect("distinct servers");
  let mcp = [
    (servers[0], version(3, server("pencil", "pencil-mcp"))),
    (servers[1], version(1, server("issues", second_arg))),
  ];
  let labels = [
    (pipeline_version.id(), label("flow")),
    (profiles[&profile].id(), label("builder")),
    (mcp[0].1.id(), label("pencil")),
    (mcp[1].1.id(), label("issues")),
  ];
  let bodies = PinnedBodies {
    profiles: &[(profiles[&profile].id(), ExecutionProfile::of_team(&team))],
    instructions: &[],
    tools: &[],
    skills: &[],
    mcp_servers: &mcp,
    labels: &labels,
  };
  let inputs = CanonicalExecutionInputs::pin(&snapshot, bodies).expect("the inputs pin");
  let bundle = ExecutionBundle::new(start(), inputs, &[(profile, AgentProvider::ClaudeCodeCli)]).expect("bundle");
  Fixture {
    profile,
    servers,
    bundle,
  }
}

fn read(value: Value) -> Result<ExecutionBundle, serde_json::Error> {
  serde_json::from_value(value)
}

#[test]
fn execution_bundle_mcp_wire_lists_the_servers_by_id_with_the_other_pinned_resources() {
  assert_eq!(EXECUTION_BUNDLE_SCHEMA, 3);
  let Fixture {
    profile,
    servers,
    bundle,
  } = fixture("issues-mcp");
  let value = serde_json::to_value(&bundle).expect("serializes");

  assert_eq!(value["schema_version"], json!(3));
  assert_eq!(
    value["resources"],
    json!([{"resource_id": servers[0], "version": 3}, {"resource_id": servers[1], "version": 1}])
  );
  assert_eq!(value["inputs"]["profiles"][0]["body"]["mcp_servers"], json!(servers));
  assert_eq!(value["inputs"]["mcp_servers"][0]["resource"], json!(servers[0]));
  assert_eq!(
    value["inputs"]["mcp_servers"][0]["label"],
    json!({"slug": "pencil", "name": "pencil"})
  );
  assert_eq!(
    value["inputs"]["mcp_servers"][1]["body"],
    json!({"name": "issues",
      "launch": {"kind": "program", "program": "npx", "args": ["-y", "issues-mcp"], "variables": ["TOKEN"]},
      "exempt_tools": ["read"]})
  );
  assert_eq!(
    value["profiles"],
    json!([{"role": "Builder", "resource_id": profile, "version": 2}])
  );
  assert_eq!(value["bindings"][0]["resource_id"], json!(profile));
  let read_back = read(value).expect("reads");
  assert_eq!(read_back, bundle);
}

#[test]
fn execution_bundle_mcp_wire_binds_the_servers_into_the_hash() {
  let Fixture { bundle, .. } = fixture("issues-mcp");
  let hash = bundle.sha256().expect("hashes");
  assert_eq!(
    fixture("issues-mcp").bundle.sha256().expect("hashes").as_str().len(),
    64
  );
  let value = serde_json::to_value(&bundle).expect("serializes");
  let reference = bundle.bundle_ref().expect("a reference");
  assert_eq!((reference.sha256, reference.schema_version), (hash.clone(), 3));

  let mut reordered = value.clone();
  reordered["inputs"]["mcp_servers"]
    .as_array_mut()
    .expect("a list")
    .reverse();
  reordered["resources"].as_array_mut().expect("a list").reverse();
  assert_eq!(read(reordered).expect("reads").sha256().expect("hashes"), hash);

  let changes: [(&str, Value); 5] = [
    ("/inputs/mcp_servers/1/body/launch/args", json!(["-y", "other"])),
    ("/inputs/mcp_servers/0/number", json!(4)),
    ("/inputs/mcp_servers/0/label/name", json!("Pencil")),
    ("/inputs/mcp_servers/1/body/exempt_tools", json!(["write"])),
    ("/inputs/mcp_servers/0/body/launch/variables", json!([])),
  ];
  for (pointer, changed) in changes {
    let mut value = value.clone();
    *value.pointer_mut(pointer).unwrap_or_else(|| panic!("{pointer} exists")) = changed;
    // `resources` is derived from the inputs, so a changed version number must move with it.
    if pointer.ends_with("/number") {
      value["resources"][0]["version"] = json!(4);
    }
    assert_ne!(read(value).expect("reads").sha256().expect("hashes"), hash, "{pointer}");
  }
  assert_ne!(fixture("issues-mcp-2").bundle.sha256().expect("hashes"), hash);
}

#[test]
fn execution_bundle_mcp_wire_refuses_what_its_inputs_do_not_decide() {
  let Fixture { servers, bundle, .. } = fixture("issues-mcp");
  let value = serde_json::to_value(bundle).expect("serializes");

  let mut other_version = value.clone();
  other_version["resources"][0]["version"] = json!(9);
  assert!(
    read(other_version)
      .expect_err("refused")
      .to_string()
      .contains("`resources`")
  );

  let mut dropped = value.clone();
  dropped["resources"].as_array_mut().expect("a list").pop();
  assert!(read(dropped).expect_err("refused").to_string().contains("`resources`"));

  let mut unpinned = value.clone();
  unpinned["inputs"]["mcp_servers"].as_array_mut().expect("a list").pop();
  assert!(read(unpinned).expect_err("refused").to_string().contains("MCP servers"));

  let mut renamed = value.clone();
  renamed["inputs"]["mcp_servers"][0]["resource"] = json!(ResourceId::generate());
  assert!(read(renamed).expect_err("refused").to_string().contains("MCP servers"));

  for (pointer, field) in [
    ("/inputs/mcp_servers/0/body", "path"),
    ("/inputs/mcp_servers/0/body/launch", "value"),
    ("/inputs/mcp_servers/0", "connection"),
  ] {
    let mut value = value.clone();
    value
      .pointer_mut(pointer)
      .and_then(Value::as_object_mut)
      .expect("an object")
      .insert(field.into(), json!("x"));
    assert!(
      read(value).expect_err("refused").to_string().contains("unknown field"),
      "{pointer}"
    );
  }
  assert_eq!(servers.len(), 2);
}

/// A bundle of the schema before this one (2) is refused, the way schema 1 was when 2 was made.
#[test]
fn execution_bundle_mcp_wire_refuses_the_schema_before_it() {
  let Fixture { bundle, .. } = fixture("issues-mcp");
  let mut value = serde_json::to_value(bundle).expect("serializes");
  value["schema_version"] = json!(2);
  let refused = read(value).expect_err("refused").to_string();
  assert!(
    refused.contains(&ExecutionBundleError::Schema(2).to_string()),
    "{refused}"
  );
  assert!(refused.contains("schema 2 is not 3"), "{refused}");
}
