//! The team MCP servers an execution bundle's inputs pin (ORC-880): the versions the pinned profiles name, by server
//! resource id, with a body of names only; hashed and read back like the rest of the inputs.

use std::{
  collections::{BTreeMap, BTreeSet},
  num::NonZeroU32,
};

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId, VersionId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
    TeamAgentProfile, TeamMcpServer, Version, VersionNumber,
  },
  run::{
    PublishedVersions, RunSnapshot,
    bundle::{BundleError, CanonicalExecutionInputs, ExecutionProfile, PinnedBodies, ResourceLabel, canonical_bytes},
  },
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

fn model() -> orchester_core::resource::ModelId {
  orchester_core::resource::ModelId::new("model-r").expect("model")
}

fn server(name: &str, program: &str) -> TeamMcpServer {
  serde_json::from_value(json!({
    "name": name,
    "launch": {"kind": "program", "program": program, "args": ["-y"], "variables": ["TOKEN"]},
    "exempt_tools": ["read"]
  }))
  .expect("a team MCP server")
}

fn remote(name: &str) -> TeamMcpServer {
  serde_json::from_value(json!({
    "name": name,
    "launch": {"kind": "remote", "url": "https://mcp.example.org/x", "secret_headers": ["X-API-Key"]}
  }))
  .expect("a team MCP server")
}

/// Two agent steps run by two profiles; the team profile bodies name the servers given for each.
struct Config {
  ids: [ResourceId; 2],
  pipeline_id: ResourceId,
  pipeline: Version<Pipeline>,
  profiles: BTreeMap<ResourceId, Version<AgentProfile>>,
  named: [Vec<ResourceId>; 2],
}

impl Config {
  fn new(named: [Vec<ResourceId>; 2]) -> Self {
    let ids = [ResourceId::generate(), ResourceId::generate()];
    let step = |key: &str, profile| Step {
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
    };
    let local = |role: &str| {
      AgentProfile::new(
        Name::new(role).expect("role"),
        "Works.".into(),
        PromptText::new("Work.").expect("prompt"),
        ConnectionId::generate(),
        model(),
        ProviderSettings::default(),
      )
    };
    Self {
      ids,
      pipeline_id: ResourceId::generate(),
      pipeline: version(
        1,
        Pipeline::new(vec![step("first", ids[0]), step("second", ids[1])]).expect("pipeline"),
      ),
      profiles: BTreeMap::from([
        (ids[0], version(1, local("first"))),
        (ids[1], version(1, local("second"))),
      ]),
      named,
    }
  }

  /// The profile as the team server holds it, naming `servers`.
  fn team_profile(role: &str, servers: &[ResourceId]) -> ExecutionProfile {
    let team = TeamAgentProfile::new(
      Name::new(role).expect("role"),
      "Works.".into(),
      PromptText::new("Work.").expect("prompt"),
      (AgentProvider::ClaudeCodeCli, model(), ProviderSettings::default()),
    )
    .with_mcp_servers(servers.to_vec())
    .expect("distinct servers");
    ExecutionProfile::of_team(&team)
  }

  fn labels(&self, servers: &[(ResourceId, Version<TeamMcpServer>)]) -> Vec<(VersionId, ResourceLabel)> {
    let mut labels = vec![
      (self.pipeline.id(), label("flow")),
      (self.profiles[&self.ids[0]].id(), label("first")),
      (self.profiles[&self.ids[1]].id(), label("second")),
    ];
    labels.extend(
      servers
        .iter()
        .map(|(id, version)| (version.id(), label(&format!("server-{}", &id.to_string()[..8])))),
    );
    labels
  }

  fn inputs(&self, servers: &[(ResourceId, Version<TeamMcpServer>)]) -> Result<CanonicalExecutionInputs, BundleError> {
    let published = PublishedVersions {
      profiles: &self.profiles,
      tools: &BTreeMap::new(),
      instructions: &BTreeMap::new(),
      skills: &BTreeMap::new(),
    };
    let snapshot = RunSnapshot::build(self.pipeline_id, &self.pipeline, published).expect("snapshot");
    let profiles = [
      (
        self.profiles[&self.ids[0]].id(),
        Self::team_profile("first", &self.named[0]),
      ),
      (
        self.profiles[&self.ids[1]].id(),
        Self::team_profile("second", &self.named[1]),
      ),
    ];
    let bodies = PinnedBodies {
      profiles: &profiles,
      instructions: &[],
      tools: &[],
      skills: &[],
      mcp_servers: servers,
      labels: &self.labels(servers),
    };
    CanonicalExecutionInputs::pin(&snapshot, bodies)
  }
}

/// Three servers `[a, b, c]` in resource id order; the first profile names `a` and `b`, the second `b` and `c`.
fn three_servers() -> (Config, Vec<(ResourceId, Version<TeamMcpServer>)>) {
  let mut ids = [ResourceId::generate(), ResourceId::generate(), ResourceId::generate()];
  ids.sort();
  let servers = vec![
    (ids[0], version(2, server("pencil", "npx"))),
    (ids[1], version(1, remote("issues"))),
    (ids[2], version(4, server("browser", "uvx"))),
  ];
  (Config::new([vec![ids[1], ids[0]], vec![ids[1], ids[2]]]), servers)
}

fn read(value: Value) -> Result<CanonicalExecutionInputs, serde_json::Error> {
  serde_json::from_value(value)
}

#[test]
fn execution_bundle_mcp_pins_each_named_server_once_in_resource_id_order() {
  let (config, servers) = three_servers();
  let inputs = config.inputs(&servers).expect("the inputs pin");

  let pinned = inputs.mcp_servers();
  assert_eq!(
    pinned.iter().map(|server| server.pinned().resource).collect::<Vec<_>>(),
    servers.iter().map(|(id, _)| *id).collect::<Vec<_>>()
  );
  assert_eq!(pinned[0].pinned().number, servers[0].1.number());
  assert_eq!(pinned[0].pinned().version, servers[0].1.id());
  assert_eq!(pinned[2].body(), servers[2].1.body());
  assert_eq!(
    pinned[1].label(),
    &label(&format!("server-{}", &servers[1].0.to_string()[..8]))
  );
  for (id, _) in &servers {
    assert!(inputs.resources().contains(id));
    assert!(inputs.label(*id).is_some());
  }
  let profiles = inputs.profiles();
  let named: Vec<&[ResourceId]> = profiles.iter().map(|pinned| pinned.body().mcp_servers()).collect();
  assert_eq!(named.len(), 2);
  assert!(
    named.contains(&&[servers[1].0, servers[0].0][..]),
    "a profile keeps its own order"
  );
}

#[test]
fn execution_bundle_mcp_pins_only_the_servers_the_profiles_name() {
  let (config, mut servers) = three_servers();
  servers.push((ResourceId::generate(), version(1, server("unused", "npx"))));
  let inputs = config.inputs(&servers).expect("the inputs pin");
  assert_eq!(inputs.mcp_servers().len(), 3);

  let none = Config::new([Vec::new(), Vec::new()]);
  let inputs = none.inputs(&servers).expect("the inputs pin");
  assert!(inputs.mcp_servers().is_empty());
}

#[test]
fn execution_bundle_mcp_refuses_a_named_server_whose_version_was_not_given() {
  let (config, servers) = three_servers();
  let missing = servers[1].0;
  let rest: Vec<_> = servers.into_iter().filter(|(id, _)| *id != missing).collect();
  assert_eq!(config.inputs(&rest), Err(BundleError::McpServerMissing(missing)));
}

#[test]
fn execution_bundle_mcp_leaves_a_local_profile_and_the_inputs_without_servers() {
  let config = Config::new([Vec::new(), Vec::new()]);
  let inputs = config.inputs(&[]).expect("the inputs pin");
  let value = serde_json::to_value(&inputs).expect("serializes");
  assert!(value.get("mcp_servers").is_none(), "{value}");
  assert!(value["profiles"][0]["body"].get("mcp_servers").is_none(), "{value}");

  let local = ExecutionProfile::of(config.profiles[&config.ids[0]].body());
  assert!(local.mcp_servers().is_empty());
  assert_eq!(
    serde_json::to_value(&local).expect("serializes").get("mcp_servers"),
    None
  );
}

/// Inputs stored before the bundle carried servers have no `mcp_servers` field and read as ones with none.
#[test]
fn execution_bundle_mcp_reads_inputs_written_before_servers_were_pinned() {
  let config = Config::new([Vec::new(), Vec::new()]);
  let inputs = config.inputs(&[]).expect("the inputs pin");
  let value = serde_json::to_value(&inputs).expect("serializes");
  assert!(value.get("mcp_servers").is_none());
  let read_back = read(value).expect("reads");
  assert_eq!(read_back, inputs);
  assert_eq!(read_back.sha256().expect("hashes"), inputs.sha256().expect("hashes"));
}

#[test]
fn execution_bundle_mcp_canonical_bytes_put_the_servers_last_with_names_only() {
  let (config, servers) = three_servers();
  let inputs = config.inputs(&servers).expect("the inputs pin");
  let bytes = String::from_utf8(canonical_bytes(&inputs).expect("canonical")).expect("UTF-8");

  let skills = bytes.find("\"skills\":[").expect("skills are sent");
  let servers_at = bytes.find("\"mcp_servers\":[{\"resource\":").expect("servers are sent");
  assert!(skills < servers_at, "{bytes}");
  assert!(bytes.ends_with("}}]}"), "the servers are the last field: {bytes}");
  let value = serde_json::to_value(&inputs).expect("serializes");
  let first = &value["mcp_servers"][0];
  assert_eq!(first["number"], json!(2));
  assert_eq!(
    first["body"],
    json!({"name": "pencil",
      "launch": {"kind": "program", "program": "npx", "args": ["-y"], "variables": ["TOKEN"]},
      "exempt_tools": ["read"]})
  );
  assert_eq!(
    value["mcp_servers"][1]["body"]["launch"],
    json!({"kind": "remote", "url": "https://mcp.example.org/x", "secret_headers": ["X-API-Key"]})
  );
  let fields: Vec<&str> = first
    .as_object()
    .expect("an object")
    .keys()
    .map(String::as_str)
    .collect();
  assert_eq!(fields, ["body", "label", "number", "resource", "version"]);
  assert!(bytes.find("\"resource\"").expect("sent") < bytes.find("\"body\"").expect("sent"));
}

#[test]
fn execution_bundle_mcp_hashes_the_same_whatever_order_the_servers_come_in() {
  let (config, servers) = three_servers();
  let sorted = config.inputs(&servers).expect("the inputs pin");
  let mut reversed = servers;
  reversed.reverse();
  let hash = sorted.sha256().expect("hashes");
  assert_eq!(
    config
      .inputs(&reversed)
      .expect("the inputs pin")
      .sha256()
      .expect("hashes"),
    hash
  );

  let mut value = serde_json::to_value(&sorted).expect("serializes");
  value["mcp_servers"].as_array_mut().expect("a list").reverse();
  let read_back = read(value).expect("reads in any order");
  assert_eq!(read_back, sorted);
  assert_eq!(read_back.sha256().expect("hashes"), hash);
}

#[test]
fn execution_bundle_mcp_hashes_differently_when_a_server_or_the_names_change() {
  let (config, servers) = three_servers();
  let inputs = config.inputs(&servers).expect("the inputs pin");
  let hash = inputs.sha256().expect("hashes");
  let value = serde_json::to_value(&inputs).expect("serializes");

  let changes: [(&str, Value); 8] = [
    ("/mcp_servers/0/body/name", json!("design")),
    ("/mcp_servers/0/body/launch/program", json!("uvx")),
    ("/mcp_servers/0/body/launch/args", json!(["-y", "--quiet"])),
    ("/mcp_servers/0/body/launch/variables", json!(["TOKEN", "OTHER"])),
    ("/mcp_servers/0/body/exempt_tools", json!(["write"])),
    ("/mcp_servers/0/number", json!(3)),
    ("/mcp_servers/0/label/slug", json!("design")),
    ("/mcp_servers/1/body/launch/secret_headers", json!(["X-Other"])),
  ];
  for (pointer, changed) in changes {
    let mut value = value.clone();
    *value.pointer_mut(pointer).unwrap_or_else(|| panic!("{pointer} exists")) = changed;
    assert_ne!(read(value).expect("reads").sha256().expect("hashes"), hash, "{pointer}");
  }

  let other = Config::new([config.named[0].clone(), vec![servers[1].0]]);
  let fewer: Vec<_> = servers.iter().take(2).cloned().collect();
  let narrowed = other.inputs(&fewer).expect("the inputs pin");
  assert_ne!(narrowed.sha256().expect("hashes"), hash);
}

#[test]
fn execution_bundle_mcp_refuses_servers_the_profiles_do_not_name_and_the_reverse() {
  let (config, servers) = three_servers();
  let value = serde_json::to_value(config.inputs(&servers).expect("the inputs pin")).expect("serializes");
  let mismatch = BundleError::McpServersMismatch.to_string();

  let mut unnamed = value.clone();
  unnamed["mcp_servers"].as_array_mut().expect("a list").pop();
  assert!(read(unnamed).expect_err("refused").to_string().contains(&mismatch));

  let mut foreign = value.clone();
  foreign["mcp_servers"][0]["resource"] = json!(ResourceId::generate());
  assert!(read(foreign).expect_err("refused").to_string().contains(&mismatch));

  let mut stray = value.clone();
  let extra = stray["mcp_servers"][0].clone();
  let mut extra = extra;
  extra["resource"] = json!(ResourceId::generate());
  stray["mcp_servers"].as_array_mut().expect("a list").push(extra);
  assert!(read(stray).expect_err("refused").to_string().contains(&mismatch));

  let mut none = value;
  none.as_object_mut().expect("an object").remove("mcp_servers");
  assert!(read(none).expect_err("refused").to_string().contains(&mismatch));
}

#[test]
fn execution_bundle_mcp_refuses_a_server_pinned_twice_or_listed_twice() {
  let (config, servers) = three_servers();
  let value = serde_json::to_value(config.inputs(&servers).expect("the inputs pin")).expect("serializes");

  let mut twice = value.clone();
  let again = twice["mcp_servers"][0].clone();
  twice["mcp_servers"].as_array_mut().expect("a list").push(again);
  let refused = read(twice).expect_err("refused").to_string();
  assert!(refused.contains("is pinned more than once"), "{refused}");

  let mut listed = value;
  let server = listed["mcp_servers"][0]["resource"].clone();
  let profile = listed["profiles"]
    .as_array_mut()
    .expect("a list")
    .iter_mut()
    .find(|profile| {
      profile["body"]["mcp_servers"]
        .as_array()
        .is_some_and(|list| list.contains(&server))
    })
    .expect("a profile names it");
  profile["body"]["mcp_servers"]
    .as_array_mut()
    .expect("a list")
    .push(server);
  let refused = read(listed).expect_err("refused").to_string();
  assert!(refused.contains("is pinned more than once"), "{refused}");
}

#[test]
fn execution_bundle_mcp_refuses_a_server_body_with_a_path_a_value_or_an_id() {
  let (config, servers) = three_servers();
  let value = serde_json::to_value(config.inputs(&servers).expect("the inputs pin")).expect("serializes");
  let edits: [(&str, &str, Value); 5] = [
    ("/mcp_servers/0/body", "path", json!("/usr/bin/npx")),
    ("/mcp_servers/0/body", "env", json!({"TOKEN": "s3cret"})),
    ("/mcp_servers/0/body/launch", "value", json!("s3cret")),
    ("/mcp_servers/0/body/launch", "id", json!("0191c7c0")),
    ("/mcp_servers/0", "connection", json!("0191c7c0")),
  ];
  for (pointer, field, extra) in edits {
    let mut value = value.clone();
    value
      .pointer_mut(pointer)
      .and_then(Value::as_object_mut)
      .expect("an object")
      .insert(field.into(), extra);
    assert!(
      read(value).expect_err("refused").to_string().contains("unknown field"),
      "{pointer} {field}"
    );
  }
  let mut path = value;
  path["mcp_servers"][0]["body"]["launch"]["program"] = json!("/usr/bin/npx");
  assert!(read(path).is_err());
}
