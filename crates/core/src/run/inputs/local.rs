//! What this computer adds to a run's canonical inputs: how it binds them to its own connections, accounts, MCP servers
//! and secrets, and what a person here narrowed or consented to. Kept apart from the canonical content, so a local
//! choice never reads as a different publication. References only: an id is never a secret value.

use serde::{Deserialize, Serialize};

use crate::{
  connection::AgentProvider,
  id::{AgentAccountId, ConnectionId, ResourceId, SecretId},
  run::{PinnedVersion, bundle::ResolvedStepPermissions},
};

/// `{resource, connection, account?, required_cli?}`: the local connection and account a pinned profile runs with,
/// and the CLI kind its shared reference requires.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileBinding {
  pub resource: ResourceId,
  pub connection: ConnectionId,
  pub account: Option<AgentAccountId>,
  /// The shared reference's required CLI; absent in runs recorded before kind validation and in local runs.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub required_cli: Option<AgentProvider>,
}

/// A connection, MCP server or secret the canonical bodies name by the publisher's id, bound to this computer's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BoundReference {
  Connection { named: ConnectionId, local: ConnectionId },
  McpServer { named: ConnectionId, local: ConnectionId },
  Secret { named: SecretId, local: SecretId },
}

/// `{profiles, references}`: how this computer binds the run's inputs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalBindings {
  /// One per pinned agent profile.
  pub profiles: Vec<ProfileBinding>,
  pub references: Vec<BoundReference>,
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
