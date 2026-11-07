use orchester_api::{
  ReferenceTarget, ResourceCreate, ResourceCreated, ResourcePatch, Resources, ResourcesImport, ResourcesImported,
  SharedResource, SharedResourceKind, SharedVersion, UploadResource, UsedBy, VersionPublish, Versions,
};
use orchester_core::{
  connection::AgentProvider,
  id::{ResourceId, RunId},
  resource::ResourceKind,
  secret::SecretName,
};
use serde_json::json;

use crate::fixtures::{hash, key, references, resource, round_trip, user, version};

#[test]
fn resources_a_resource_of_every_shared_kind_round_trips() {
  for kind in [
    "pipeline",
    "agent_profile",
    "instruction",
    "skill",
    "tool",
    "team_reviewer",
  ] {
    let shared: SharedResource = round_trip(&resource(ResourceId::generate(), kind));
    assert_eq!(SharedResourceKind::from_core(shared.kind.core()), Some(shared.kind));
  }
  let mut never_published = resource(ResourceId::generate(), "skill");
  never_published["latest"] = json!(null);
  assert_eq!(round_trip::<SharedResource>(&never_published).latest, None);
}

#[test]
fn resources_agent_testing_kinds_are_not_shared_kinds() {
  for (kind, name) in [
    (ResourceKind::TestSuite, "test_suite"),
    (ResourceKind::Service, "service"),
    (ResourceKind::Stack, "stack"),
  ] {
    assert_eq!(SharedResourceKind::from_core(kind), None);
    let local = resource(ResourceId::generate(), name);
    assert!(serde_json::from_value::<SharedResource>(local).is_err());
  }
}

#[test]
fn resources_a_version_round_trips_with_a_reference_of_each_kind() {
  let id = ResourceId::generate();
  let first: SharedVersion = round_trip(&version(id, 1));
  assert_eq!(first.based_on, None);
  let third: SharedVersion = round_trip(&version(id, 3));
  assert_eq!(third.based_on.map(|number| number.get()), Some(2));
  let kinds: Vec<_> = third
    .references
    .iter()
    .map(|reference| match reference {
      ReferenceTarget::CliKind { .. } => "cli_kind",
      ReferenceTarget::TeamResource {
        kind: SharedResourceKind::McpServer,
        ..
      } => "mcp_server",
      ReferenceTarget::TeamResource {
        kind: SharedResourceKind::Tool,
        ..
      } => "tool",
      ReferenceTarget::TeamResource { .. } => "other team resource",
      ReferenceTarget::SecretName { .. } => "secret_name",
    })
    .collect();
  assert_eq!(kinds, ["cli_kind", "mcp_server", "secret_name", "tool"]);
}

#[test]
fn resources_a_reference_names_a_cli_kind_a_team_resource_by_server_id_or_a_secret_by_name() {
  let profile = ResourceId::generate();
  let cli: ReferenceTarget = round_trip(&json!({"type":"cli_kind","cli":"codex_cli"}));
  assert_eq!(
    cli,
    ReferenceTarget::CliKind {
      cli: AgentProvider::CodexCli
    }
  );
  let resource: ReferenceTarget = round_trip(&json!({"type":"team_resource","kind":"agent_profile","id":profile}));
  assert_eq!(
    resource,
    ReferenceTarget::TeamResource {
      kind: SharedResourceKind::AgentProfile,
      id: profile
    }
  );
  let secret: ReferenceTarget = round_trip(&json!({"type":"secret_name","name":"STAGING_TOKEN"}));
  assert_eq!(
    secret,
    ReferenceTarget::SecretName {
      name: SecretName::new("STAGING_TOKEN").expect("a secret name")
    }
  );
}

#[test]
fn resources_a_reference_cannot_hold_an_id_or_a_path_from_a_computer() {
  let local = ResourceId::generate();
  for value in [
    // The shapes before the team configuration model: a connection, an MCP server or a secret by a computer's id.
    json!({"kind":"connection","id":local,"name":"Work","role":"codex_cli"}),
    json!({"kind":"mcp_server","id":local,"name":"docs","role":null}),
    json!({"kind":"secret","id":local,"name":"Token","role":"token"}),
    json!({"kind":"tool","id":local,"name":"Deploy","role":null}),
    json!({"type":"connection","id":local}),
    json!({"type":"mcp_server","id":local}),
    json!({"type":"secret","id":local}),
    // Known types with what they need missing, or more than they hold.
    json!({"type":"cli_kind"}),
    json!({"type":"cli_kind","cli":"github"}),
    json!({"type":"team_resource","id":local}),
    json!({"type":"team_resource","kind":"test_suite","id":local}),
    json!({"type":"team_resource","kind":"tool"}),
    json!({"type":"team_resource","kind":"tool","id":"not-an-id"}),
    json!({"type":"team_resource","kind":"tool","id":local,"name":"Deploy"}),
    json!({"type":"secret_name"}),
    json!({"type":"secret_name","name":"lowercase"}),
    json!({"type":"secret_name","name":""}),
    json!({"type":"secret_name","name":"TOKEN","id":local}),
    json!({"type":"environment","name":"TOKEN"}),
    json!({"cli":"codex_cli"}),
  ] {
    assert!(
      serde_json::from_value::<ReferenceTarget>(value.clone()).is_err(),
      "{value}"
    );
  }
}

#[test]
fn resources_a_version_hash_is_a_content_hash() {
  let mut bad = version(ResourceId::generate(), 1);
  bad["hash"] = json!("not-a-hash");
  assert!(serde_json::from_value::<SharedVersion>(bad).is_err());
}

#[test]
fn resources_a_create_and_its_answer_round_trip() {
  let id = ResourceId::generate();
  let create: ResourceCreate = round_trip(&json!({"idempotency_key":key(),"id":id,"kind":"agent_profile",
    "slug":"reviewer","name":"Reviewer","version":{"body":{"name":"Reviewer"},"references":references()}}));
  assert_eq!(create.kind, SharedResourceKind::AgentProfile);
  round_trip::<ResourceCreated>(&json!({"resource":resource(id, "agent_profile"),"version":version(id, 1)}));
}

#[test]
fn resources_an_import_and_its_answer_round_trip() {
  let id = ResourceId::generate();
  let upload = json!({"id":id,"kind":"skill","slug":"rust","name":"Rust","archived":true,"versions":[
    {"number":1,"body":{"files":[{"path":"SKILL.md","hash":hash('b'),"size":12,"executable":false}]},
      "references":[],"published_at":1_792_000_000_000_i64},
    {"number":2,"body":{"files":[]},"references":references(),"published_at":1_792_000_000_001_i64},
  ]});
  let uploaded: UploadResource = round_trip(&upload);
  assert_eq!(uploaded.versions.len(), 2);
  round_trip::<ResourcesImport>(&json!({"idempotency_key":key(),"resources":[upload]}));
  round_trip::<ResourcesImported>(&json!({"resources":[resource(id, "skill")]}));
}

#[test]
fn resources_a_publish_round_trips_from_a_base_and_from_none() {
  for based_on in [json!(4), json!(null)] {
    let publish: VersionPublish = round_trip(&json!({"idempotency_key":key(),"based_on":based_on,
      "body":{"name":"Feature delivery"},"references":references()}));
    assert_eq!(publish.references.len(), 4);
  }
}

#[test]
fn resources_a_patch_sends_only_what_changes() {
  for changes in [
    json!({"name":"Renamed"}),
    json!({"archived":true}),
    json!({"name":"Renamed","archived":false}),
  ] {
    let mut patch = json!({"idempotency_key":key(),"revision":3});
    for (field, value) in changes.as_object().expect("changes") {
      patch[field] = value.clone();
    }
    round_trip::<ResourcePatch>(&patch);
  }
}

#[test]
fn resources_a_request_refuses_fields_it_does_not_know() {
  let unknown = json!({"idempotency_key":key(),"revision":3,"slug":"renamed"});
  assert!(serde_json::from_value::<ResourcePatch>(unknown).is_err());
  let unknown = json!({"idempotency_key":key(),"based_on":1,"body":{},"references":[],"number":2});
  assert!(serde_json::from_value::<VersionPublish>(unknown).is_err());
}

#[test]
fn resources_the_lists_round_trip_with_their_cursor_and_next_page() {
  let id = ResourceId::generate();
  round_trip::<Resources>(&json!({"resources":[resource(id, "tool")],"cursor":42}));
  let last: Versions = round_trip(&json!({"versions":[version(id, 1), version(id, 2)]}));
  assert_eq!(last.next, None);
  let page: Versions = round_trip(&json!({"versions":[version(id, 1)],"next":1}));
  assert_eq!(page.next.map(|number| number.get()), Some(1));
}

#[test]
fn resources_used_by_round_trips_and_an_unused_resource_has_two_empty_lists() {
  let used = json!({
    "resources":[{"kind":"agent_profile","id":ResourceId::generate(),"name":"Reviewer","version":2}],
    "runs":[
      {"task_key":"KON-7","run":RunId::generate(),"number":1,"holder":user()},
      {"task_key":"KON-8","run":RunId::generate(),"number":1},
    ],
  });
  let decoded: UsedBy = round_trip(&used);
  assert_eq!(decoded.resources[0].kind, SharedResourceKind::AgentProfile);
  assert_eq!(decoded.runs[0].task_key, "KON-7");
  assert!(decoded.runs[0].holder.is_some(), "a held task names its holder");
  assert!(
    decoded.runs[1].holder.is_none(),
    "a released claim leaves the holder absent"
  );
  let unused: UsedBy = round_trip(&json!({"resources":[],"runs":[]}));
  assert!(unused.resources.is_empty() && unused.runs.is_empty());
  assert!(serde_json::from_value::<UsedBy>(json!({"resources":[]})).is_err());
}
