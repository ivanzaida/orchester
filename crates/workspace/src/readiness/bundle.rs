//! Whether this computer is ready for the versions an execution bundle pins, which the team server may have pinned
//! newer than the ones cached here when readiness was read before the start. The same rule as [`super::read`], over the
//! pinned profiles (the CLI the bundle binds each to, the MCP servers it names) and the pinned versions of those MCP
//! servers, against what this computer has now.

use orchester_api::v2::ExecutionBundle;
use orchester_core::{
  id::ResourceId,
  sharing::{McpNeed, Missing, Needs, readiness},
};
use orchester_store::{ProjectDb, StoreError};

use super::{facts, not_ready::MissingPart, programs::McpPrograms};

/// What `bundle` needs that this computer lacks, in the order to fix it; empty when it is ready.
pub(crate) fn missing(
  db: &ProjectDb,
  programs: &McpPrograms,
  bundle: &ExecutionBundle,
) -> Result<Vec<MissingPart>, StoreError> {
  let inputs = bundle.inputs();
  let servers: Vec<McpNeed> = inputs
    .mcp_servers()
    .iter()
    .map(|pinned| McpNeed::of(pinned.pinned().resource, pinned.body()))
    .collect();
  let server_ids: Vec<ResourceId> = inputs
    .mcp_servers()
    .iter()
    .map(|pinned| pinned.pinned().resource)
    .collect();
  let mut here = facts::with_programs(facts::base(db)?, programs, &server_ids);
  for server in &server_ids {
    if db.team_mcp_switch(*server)? == Some(true) {
      here = here.with_mcp_on(*server);
    }
  }
  let mut needs = Vec::new();
  for binding in bundle.bindings() {
    if let Some(pick) = db.connection_pick(binding.resource_id)? {
      here = here.with_pick(binding.resource_id, pick.connection);
    }
    let chosen = inputs
      .profiles()
      .iter()
      .find(|pinned| pinned.pinned().resource == binding.resource_id)
      .map_or(&[][..], |pinned| pinned.body().mcp_servers());
    needs.push(
      Needs::pinned_profile(binding.resource_id, (binding.cli, chosen), &servers)
        .expect("a bundle pins the version of every MCP server its profiles name"),
    );
  }
  let names = |resource: ResourceId| name(bundle, resource);
  Ok(
    readiness(&Needs::union(needs), &here)
      .missing()
      .iter()
      .map(|missing| MissingPart::new(Missing::clone(missing), &names))
      .collect(),
  )
}

/// The name the bundle gives `resource`: a profile's role or an MCP server's name; its id when it pins neither.
fn name(bundle: &ExecutionBundle, resource: ResourceId) -> String {
  let profile = bundle
    .bindings()
    .iter()
    .find(|binding| binding.resource_id == resource)
    .map(|binding| binding.role.as_str().to_owned());
  let server = bundle
    .inputs()
    .mcp_servers()
    .iter()
    .find(|pinned| pinned.pinned().resource == resource)
    .map(|pinned| pinned.body().name().as_str().to_owned());
  profile.or(server).unwrap_or_else(|| resource.to_string())
}
