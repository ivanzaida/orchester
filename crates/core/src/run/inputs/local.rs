//! What this computer adds to a run's canonical inputs: how it binds them to its own connections, accounts, MCP servers
//! and secrets, and what a person here narrowed or consented to. Kept apart from the canonical content, so a local
//! choice never reads as a different publication. References only: an id is never a secret value.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
  connection::AgentProvider,
  id::{AgentAccountId, ConnectionId, ResourceId},
  run::{PinnedVersion, bundle::ResolvedStepPermissions},
};

/// `{resource, connection, account?, required_cli?, mcp_servers?}`: the local connection and account a pinned profile
/// runs with, the CLI kind its shared reference requires, and the team MCP servers its version names (each also bound
/// in [`LocalBindings::team_mcp`], under [`team_mcp_id`](super::team_mcp_id)).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileBinding {
  pub resource: ResourceId,
  pub connection: ConnectionId,
  pub account: Option<AgentAccountId>,
  /// The shared reference's required CLI; absent in runs recorded before kind validation and in local runs.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub required_cli: Option<AgentProvider>,
  /// The MCP servers this profile's agents get, and only this profile's; none for a profile that names none.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub mcp_servers: Vec<ConnectionId>,
}

/// `{server, program?}`: where this computer runs a team MCP server the run's profiles name, as filled in when the run
/// started: the path of its program, for a server that runs one. A path is not a secret; the secrets' values are read
/// by name when an agent starts, never kept here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeamMcpBinding {
  /// The team MCP server's resource id, whose pinned version is among the canonical inputs.
  pub server: ResourceId,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub program: Option<PathBuf>,
}

/// `{profiles, team_mcp?}`: how this computer binds the run's inputs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalBindings {
  /// One per pinned agent profile.
  pub profiles: Vec<ProfileBinding>,
  /// One per pinned team MCP server; left out while empty, which is every local run's.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub team_mcp: Vec<TeamMcpBinding>,
}

/// `{accepted, narrowed}`: what a person on this computer decided for the run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalConsent {
  /// Pinned versions whose widening of what agents may do a person accepted here (*Use v<N> here*).
  pub accepted: Vec<PinnedVersion>,
  /// Agent steps whose permissions this computer narrows below the inputs'; never wider.
  pub narrowed: Vec<ResolvedStepPermissions>,
}
