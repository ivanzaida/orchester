use orchester_api::{
  Event, ReferenceTarget, ResourceCreate, ResourceCreated, Resources, SharedResourceKind, SharedVersion, UploadVersion,
  VersionContent, VersionPublish, Versions,
};
use orchester_core::{id::ResourceId, resource::TeamReviewer};
use serde_json::{Value, json};

use crate::fixtures::{key, resource, round_trip, user, version};

fn decision(threshold: f64, hosts: &[&str]) -> Value {
  json!({
    "kind":"decision",
    "model":"decision-1", "lowest_threshold":threshold,
    "destinations":{"kind":"hosts","hosts":hosts}
  })
}

#[test]
fn reviewer_resource_bodies_create_publish_and_read_with_core_shapes() {
  let id = ResourceId::generate();
  let language = json!({
    "kind":"language", "cli":"codex_cli", "model":"language-1", "effort":"high"
  });
  for body in [language, decision(0.8, &["review.example.org"])] {
    let reviewer = round_trip::<TeamReviewer>(&body);
    let references = serde_json::to_value(ReferenceTarget::of_reviewer(&reviewer)).expect("references");
    let content = json!({"body":body,"references":references});
    round_trip::<VersionContent>(&content);
    let create: ResourceCreate = round_trip(&json!({
      "idempotency_key":key(),"id":id,"kind":"team_reviewer","slug":"reviewer","name":"Team reviewer",
      "version":content
    }));
    assert_eq!(create.kind, SharedResourceKind::TeamReviewer);
    round_trip::<VersionPublish>(&json!({
      "idempotency_key":key(),"based_on":1,"body":body,"references":references
    }));
    let mut published = version(id, 2);
    published["body"] = body;
    published["references"] = references;
    published["widens"] = json!(true);
    let read: SharedVersion = round_trip(&published);
    assert!(read.widens);
    round_trip::<ResourceCreated>(&json!({"resource":resource(id,"team_reviewer"),"version":published}));
    round_trip::<Versions>(&json!({"versions":[published]}));
  }
  round_trip::<Resources>(&json!({"resources":[resource(id,"team_reviewer")],"cursor":42}));
}

#[test]
fn reviewer_resource_widens_is_server_owned_and_survives_version_reads() {
  let id = ResourceId::generate();
  let previous: TeamReviewer = serde_json::from_value(decision(0.8, &["review.example.org"])).expect("body");
  for (body, expected) in [
    (decision(0.8, &["review.example.org"]), false),
    (decision(0.9, &[]), false),
    (decision(0.7, &["review.example.org"]), true),
    (decision(0.8, &["other.example.org", "review.example.org"]), true),
  ] {
    let next: TeamReviewer = serde_json::from_value(body.clone()).expect("body");
    let mut published = version(id, 2);
    published["body"] = body.clone();
    published["widens"] = json!(next.widens_over(Some(&previous)));
    assert_eq!(round_trip::<SharedVersion>(&published).widens, expected);
    for flag in [true, false] {
      let content = json!({"body":body,"references":[],"widens":flag});
      assert!(serde_json::from_value::<VersionContent>(content.clone()).is_err());
      let mut publish = content.clone();
      publish["idempotency_key"] = json!(key());
      publish["based_on"] = json!(1);
      assert!(serde_json::from_value::<VersionPublish>(publish).is_err());
      let mut upload = content;
      upload["number"] = json!(1);
      upload["published_at"] = json!(3);
      assert!(serde_json::from_value::<UploadVersion>(upload).is_err());
    }
  }
  let mut legacy = version(id, 1);
  legacy.as_object_mut().expect("version").remove("widens");
  assert!(
    !serde_json::from_value::<SharedVersion>(legacy)
      .expect("older server")
      .widens
  );
}

#[test]
fn reviewer_resource_uses_existing_resource_deltas() {
  let id = ResourceId::generate();
  for (kind, change) in [
    ("resource.published", json!({"resource":resource(id,"team_reviewer")})),
    ("resource.renamed", json!({"resource":id,"name":"Renamed reviewer"})),
    ("resource.archived", json!({"resource":id,"archived":true})),
  ] {
    let mut data = json!({"id":42,"by":user(),"actor":"person","at":3});
    data
      .as_object_mut()
      .expect("delta")
      .extend(change.as_object().expect("change").clone());
    round_trip::<Event>(&json!({"kind":kind,"data":data}));
  }
}

#[test]
fn reviewer_references_are_the_cli_kind_of_a_language_reviewer_and_none_for_a_decision_one() {
  let language: TeamReviewer =
    serde_json::from_value(json!({"kind":"language","cli":"claude_code_cli","model":"m-1","effort":null}))
      .expect("body");
  let decision: TeamReviewer = serde_json::from_value(decision(0.8, &[])).expect("body");
  assert_eq!(
    serde_json::to_value(ReferenceTarget::of_reviewer(&language)).expect("serialize"),
    json!([{"type":"cli_kind","cli":"claude_code_cli"}])
  );
  assert!(ReferenceTarget::of_reviewer(&decision).is_empty());
}

#[test]
fn reviewer_bodies_that_name_a_connection_are_not_read() {
  let old = json!({
    "kind":"language", "connection":{"kind":"connection","name":"Team language","role":"codex_cli"},
    "model":"language-1", "effort":"high"
  });
  assert!(serde_json::from_value::<TeamReviewer>(old).is_err());
}
