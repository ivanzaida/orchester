use orchester_api::{
  Reference, ReferenceTarget, ResourceCreate, ResourceCreated, ResourcePatch, Resources, ResourcesImport,
  ResourcesImported, SharedResource, SharedResourceKind, SharedVersion, UploadResource, VersionPublish, Versions,
};
use orchester_core::{
  id::{ResourceId, SecretId},
  resource::ResourceKind,
};
use serde_json::json;

use crate::fixtures::{hash, key, references, resource, round_trip, version};

#[test]
fn resources_a_resource_of_every_shared_kind_round_trips() {
  for kind in ["pipeline", "agent_profile", "instruction", "skill", "tool"] {
    let shared: SharedResource = round_trip(&resource(ResourceId::generate(), kind));
    assert_eq!(SharedResourceKind::from_core(shared.kind.core()), Some(shared.kind));
  }
  let mut never_published = resource(ResourceId::generate(), "skill");
  never_published["latest"] = json!(null);
  assert_eq!(round_trip::<SharedResource>(&never_published).latest, None);
}

#[test]
fn resources_a_test_suite_is_not_a_shared_kind() {
  assert_eq!(SharedResourceKind::from_core(ResourceKind::TestSuite), None);
  let suite = resource(ResourceId::generate(), "test_suite");
  assert!(serde_json::from_value::<SharedResource>(suite).is_err());
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
    .map(|reference| match reference.target {
      ReferenceTarget::Connection { .. } => "connection",
      ReferenceTarget::McpServer { .. } => "mcp_server",
      ReferenceTarget::Secret { .. } => "secret",
      ReferenceTarget::Tool { .. } => "tool",
    })
    .collect();
  assert_eq!(kinds, ["connection", "mcp_server", "secret", "tool"]);
}

#[test]
fn resources_a_reference_needs_a_known_kind_and_its_id() {
  let secret = SecretId::generate();
  let reference: Reference = round_trip(&json!({"kind":"secret","id":secret,"name":"Token","role":null}));
  assert_eq!(reference.target, ReferenceTarget::Secret { id: secret });
  for value in [
    json!({"kind":"environment","id":secret,"name":"Token","role":null}),
    json!({"kind":"secret","name":"Token","role":null}),
    json!({"id":secret,"name":"Token","role":null}),
  ] {
    assert!(serde_json::from_value::<Reference>(value).is_err());
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
