//! What an App Server session starts with: the prompt, the thread's settings and the approvals it may ask a person
//! for, as a step (`super::super::start::cli`) or the Orchestrator chat (`crate::execution::chat::codex`) sets them.

use std::{collections::BTreeSet, path::PathBuf};

use orchester_core::{
  attachment::input::AttachedFile, command::SecretVariable, connection::mcp::InvocationExemptions,
  permission::NetworkAccess, resource::RepositoryAccess,
};
use serde_json::Value;

pub(in crate::execution) struct AppServerSetup {
  pub prompt: String,
  /// The files people attached to the messages the prompt delivers, given with it.
  pub attached: Vec<AttachedFile>,
  pub model: String,
  pub effort: Option<String>,
  pub worktree: PathBuf,
  pub access: RepositoryAccess,
  /// Commands an execpolicy rule asks about reach a person (*Ask before running commands*).
  pub command_approvals: bool,
  /// Commands that need more than the sandbox reach a person.
  pub sandbox_approvals: bool,
  pub tool_approvals: bool,
  pub network: NetworkAccess,
  pub writable_roots: Vec<PathBuf>,
  pub resume: Option<String>,
  pub expected_servers: BTreeSet<String>,
  /// The MCP tools that run without asking: empty unless the step asks before MCP actions.
  pub exemptions: InvocationExemptions,
  pub output_schema: Option<Value>,
  pub secrets: Vec<SecretVariable>,
  /// The model provider the thread runs on when the connection runs on an API provider
  /// (`docs/design/api-providers.md`): then Codex needs no sign-in of its own.
  pub model_provider: Option<String>,
  /// Instructions in place of Codex's own, given with the thread's start and resume (`baseInstructions`): the
  /// Orchestrator chat's system prompt; `None` for a step, which keeps Codex's.
  pub base_instructions: Option<String>,
}

impl AppServerSetup {
  pub(super) fn runtime_roots(&self) -> BTreeSet<PathBuf> {
    let mut roots = BTreeSet::from([self.worktree.clone()]);
    if self.access == RepositoryAccess::Write {
      roots.extend(self.writable_roots.iter().cloned());
    }
    roots
  }
}
