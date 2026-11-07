//! A team start the server refuses `bundle_missing` is answered `team_bundle_missing` with a message per reason
//! (`appMcp.errors.teamBundleMissing*`): the agent profile to publish again, the archived MCP server, or the plain
//! refusal when the server gives no reason. The resource is named as this computer has it cached, or by its id.

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Resource, ResourceKind, Slug},
  sharing::TaskRevision,
  time::Timestamp,
};
use serde_json::{Value, json};

use crate::{
  Checkout,
  bundle_schema::{refusal, started_with},
};

/// `run_task` on a checkout whose server refuses the start with `details`, after `prepare` kept what the computer has
/// cached: the refusal's code and rendered messages. The server made no work.
fn refused_with(details: Value, prepare: impl FnOnce(&Checkout)) -> (String, String) {
  let (server, checkout) = started_with(|state| state.bundle_missing = Some(details));
  prepare(&checkout);
  let (code, error) = refusal(&checkout);
  assert_eq!(server.state().works_made, 0, "the server made no work");
  (code, error.to_string())
}

fn reason(kind: &str, resource: Option<ResourceId>) -> Value {
  match resource {
    Some(id) => json!({"reason": {"kind": kind, "resource_id": id, "version": 3}}),
    None => json!({"reason": {"kind": kind}}),
  }
}

/// Keeps `id` as a team resource of `kind` named `name`, as the computer has it cached.
fn keep(checkout: &Checkout, (id, kind): (ResourceId, ResourceKind), name: &str) {
  let resource = Resource::new(
    id,
    kind,
    Slug::new(&name.to_lowercase()).expect("slug"),
    Name::new(name).expect("name"),
    Timestamp::now(),
  );
  checkout
    .db()
    .keep_shared_resource(&resource, TaskRevision::try_from(1).expect("a revision"))
    .expect("a team resource");
}

#[test]
fn team_mcp_bundle_missing_names_the_profile_to_publish_again() {
  let profile = ResourceId::generate();
  let (code, error) = refused_with(reason("profile_unreadable", Some(profile)), |checkout| {
    keep(checkout, (profile, ResourceKind::AgentProfile), "Reviewer");
  });
  assert_eq!(code, "team_bundle_missing", "{error}");
  assert!(
    error.contains("appMcp.errors.teamBundleMissingProfile profile=Reviewer"),
    "{error}"
  );
  assert!(!error.contains(&profile.to_string()), "{error}");
}

#[test]
fn team_mcp_bundle_missing_falls_back_to_the_id_of_a_profile_not_cached_here() {
  let profile = ResourceId::generate();
  let (code, error) = refused_with(reason("profile_unreadable", Some(profile)), |_| {});
  assert_eq!(code, "team_bundle_missing", "{error}");
  assert!(
    error.contains(&format!("appMcp.errors.teamBundleMissingProfile profile={profile}")),
    "{error}"
  );
}

#[test]
fn team_mcp_bundle_missing_names_the_resource_of_a_file_or_body_it_cannot_use() {
  for (kind, key) in [
    ("file_missing", "appMcp.errors.teamBundleMissingFile"),
    ("body_unreadable", "appMcp.errors.teamBundleMissingBody"),
  ] {
    let skill = ResourceId::generate();
    let (code, error) = refused_with(reason(kind, Some(skill)), |checkout| {
      keep(checkout, (skill, ResourceKind::Skill), "Lint");
    });
    assert_eq!(code, "team_bundle_missing", "{kind}: {error}");
    assert!(error.contains(&format!("{key} resource=Lint")), "{kind}: {error}");

    let unknown = ResourceId::generate();
    let (_, error) = refused_with(reason(kind, Some(unknown)), |_| {});
    assert!(error.contains(&format!("{key} resource={unknown}")), "{kind}: {error}");
  }
}

#[test]
fn team_mcp_bundle_missing_names_the_archived_mcp_server_a_profile_still_uses() {
  let server = ResourceId::generate();
  let (code, error) = refused_with(reason("mcp_server_archived", Some(server)), |checkout| {
    keep(checkout, (server, ResourceKind::McpServer), "Playwright");
  });
  assert_eq!(code, "team_bundle_missing", "{error}");
  assert!(
    error.contains("appMcp.errors.teamBundleMissingMcpArchived server=Playwright"),
    "{error}"
  );
}

#[test]
fn team_mcp_bundle_missing_names_the_pipeline_the_server_cannot_find() {
  let pipeline = ResourceId::generate();
  let (code, error) = refused_with(reason("pipeline_missing", Some(pipeline)), |_| {});
  assert_eq!(code, "team_bundle_missing", "{error}");
  assert!(
    error.contains(&format!("appMcp.errors.teamBundleMissingPipeline pipeline={pipeline}")),
    "{error}"
  );
}

#[test]
fn team_mcp_bundle_missing_has_a_message_for_every_other_reason_and_for_none() {
  for (details, key) in [
    (
      reason("bundle_too_large", None),
      "appMcp.errors.teamBundleMissingTooLarge",
    ),
    (reason("other", None), "appMcp.errors.teamBundleMissing "),
    (reason("quota_exceeded", None), "appMcp.errors.teamBundleMissing "),
    (reason("cli_kind_missing", None), "appMcp.errors.teamBundleMissing "),
    (reason("profile_unreadable", None), "appMcp.errors.teamBundleMissing "),
    (reason("file_missing", None), "appMcp.errors.teamBundleMissing "),
    (json!({"reason": {}}), "appMcp.errors.teamBundleMissing "),
    (json!({"reason": {"kind": 7}}), "appMcp.errors.teamBundleMissing "),
    (json!({}), "appMcp.errors.teamBundleMissing "),
  ] {
    let (code, error) = refused_with(details.clone(), |_| {});
    assert_eq!(code, "team_bundle_missing", "{details}: {error}");
    assert!(error.contains(key), "{details}: {error}");
  }
}
