//! The stand-in team server holds a version's references to the rules of `docs/cloud-api.md`, "Team resource
//! references": a team resource is named only if it exists in the project, is not archived and is of the kind the
//! reference says; a reference is not repeated and keeps the order it was sent in; a secret name is trimmed on read;
//! and no CLI kind is required of a profile.
//!
//! `cargo test -p orchester-api --features test-support --test team_references stand_in`

#[path = "support/team.rs"]
mod team;

use orchester_api::{SharedVersion, test_support::team_server::Standing};
use orchester_core::id::{IdempotencyKey, ResourceId};
use serde_json::{Value, json};
use team::Team;

fn server_body() -> Value {
  json!({"name":"knowledge","launch":{"kind":"program","program":"npx"}})
}

fn create(id: ResourceId, kind: &str, slug: &str, references: &Value) -> Value {
  let body = if kind == "mcp_server" { server_body() } else { json!({}) };
  json!({"idempotency_key":IdempotencyKey::generate(),"id":id,"kind":kind,"slug":slug,"name":slug,
    "version":{"body":body,"references":references}})
}

fn publish(based_on: u32, references: &Value) -> Value {
  json!({"idempotency_key":IdempotencyKey::generate(),"based_on":based_on,"body":{},"references":references})
}

fn mcp_server(id: ResourceId) -> Value {
  json!({"type":"team_resource","kind":"mcp_server","id":id})
}

fn secret(name: &str) -> Value {
  json!({"type":"secret_name","name":name})
}

fn reason(answer: &(u16, Value)) -> (u16, Option<&str>) {
  (answer.0, answer.1["reason"].as_str())
}

/// An admin's team with an MCP server `docs` and a profile `builder` at v1 without references.
fn team_with_docs() -> (Team, ResourceId, ResourceId) {
  let team = Team::start(Standing::Admin);
  let (docs, builder) = (ResourceId::generate(), ResourceId::generate());
  for made in [
    create(docs, "mcp_server", "docs", &json!([])),
    create(builder, "agent_profile", "builder", &json!([])),
  ] {
    assert_eq!(team.resources("POST", "", &made).0, 201);
  }
  (team, docs, builder)
}

#[test]
fn stand_in_accepts_a_team_resource_reference_only_for_an_existing_resource_of_that_kind_that_is_not_archived() {
  let (team, docs, builder) = team_with_docs();
  let versions = format!("/{builder}/versions");

  let accepted = team.resources("POST", &versions, &publish(1, &json!([mcp_server(docs)])));
  assert_eq!(accepted.0, 201, "{}", accepted.1);

  let unknown = team.resources(
    "POST",
    &versions,
    &publish(2, &json!([mcp_server(ResourceId::generate())])),
  );
  assert_eq!(
    reason(&unknown),
    (422, Some("references")),
    "a resource the project does not have"
  );
  let wrong_kind = json!([{"type":"team_resource","kind":"tool","id":docs}]);
  let wrong = team.resources("POST", &versions, &publish(2, &wrong_kind));
  assert_eq!(
    reason(&wrong),
    (422, Some("references")),
    "an MCP server named as a tool"
  );
  let new = ResourceId::generate();
  let self_named = team.resources(
    "POST",
    "",
    &create(
      new,
      "tool",
      "own",
      &json!([{"type":"team_resource","kind":"tool","id":new}]),
    ),
  );
  assert_eq!(
    reason(&self_named),
    (422, Some("references")),
    "a resource that does not exist yet"
  );

  let archive = json!({"idempotency_key":IdempotencyKey::generate(),"revision":1,"archived":true});
  assert_eq!(team.resources("PATCH", &format!("/{docs}"), &archive).0, 200);
  let archived = team.resources("POST", &versions, &publish(2, &json!([mcp_server(docs)])));
  assert_eq!(reason(&archived), (422, Some("references")), "an archived resource");
  assert_eq!(team.server.resources("kontur").len(), 2, "nothing refused was kept");
}

#[test]
fn stand_in_refuses_a_repeated_reference_and_keeps_the_order_it_was_sent_in() {
  let (team, docs, builder) = team_with_docs();
  let versions = format!("/{builder}/versions");

  let repeated = team.resources(
    "POST",
    &versions,
    &publish(1, &json!([mcp_server(docs), secret("TOKEN"), mcp_server(docs)])),
  );
  assert_eq!(reason(&repeated), (422, Some("references")));
  let as_read_back = team.resources(
    "POST",
    &versions,
    &publish(1, &json!([secret("TOKEN"), secret(" TOKEN ")])),
  );
  assert_eq!(
    reason(&as_read_back),
    (422, Some("references")),
    "a name read back trimmed repeats its target"
  );

  let cli = json!({"type":"cli_kind","cli":"codex_cli"});
  let sent = json!([secret("ZED_TOKEN"), cli, mcp_server(docs), secret("ALPHA_TOKEN")]);
  let (status, published) = team.resources("POST", &versions, &publish(1, &sent));
  assert_eq!(status, 201, "{published}");
  let published: SharedVersion = serde_json::from_value(published["version"].clone()).expect("a version");
  assert_eq!(
    serde_json::to_value(&published.references).expect("references"),
    sent,
    "sent order, not sorted"
  );
}

#[test]
fn stand_in_trims_a_secret_name_and_refuses_one_that_breaks_the_rule() {
  let (team, _docs, builder) = team_with_docs();
  let versions = format!("/{builder}/versions");

  let (status, published) = team.resources("POST", &versions, &publish(1, &json!([secret("  STAGING_TOKEN\n")])));
  assert_eq!(status, 201, "{published}");
  assert_eq!(
    published["version"]["references"],
    json!([secret("STAGING_TOKEN")]),
    "read back trimmed"
  );

  for name in ["staging_token", "1TOKEN", "TOKEN-2", "", &"A".repeat(129)] {
    let (status, answer) = team.resources("POST", &versions, &publish(2, &json!([secret(name)])));
    assert_eq!((status, answer["reason"].as_str()), (422, Some("shape")), "{name:?}");
  }
}

#[test]
fn stand_in_does_not_require_a_cli_kind_of_a_profile_version() {
  let (team, _docs, builder) = team_with_docs();
  let (status, published) = team.resources("POST", &format!("/{builder}/versions"), &publish(1, &json!([])));
  assert_eq!(status, 201, "{published}");
  assert_eq!(
    published["version"]["references"],
    json!([]),
    "a desktop sends none until ORC-820 and ORC-830"
  );
}
