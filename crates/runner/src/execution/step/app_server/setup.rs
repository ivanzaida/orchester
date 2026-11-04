//! What an App Server session starts with: the prompt, the thread's settings and the approvals it may ask a person
//! for, as a step (`super::super::start::cli`) or the Orchestrator chat (`crate::execution::chat::codex`) sets them.

use std::{
  collections::{BTreeMap, BTreeSet},
  path::PathBuf,
};

use orchester_core::{
  attachment::input::AttachedFile,
  command::SecretVariable,
  connection::{invocation::CodexChatSkill, mcp::InvocationExemptions},
  permission::NetworkAccess,
  resource::RepositoryAccess,
};
use serde_json::Value;

/// A skill the Orchestrator chat's turn loads as Codex's own (ORC-663): its folder, which the session registers before
/// the thread starts, and the person's words, which the turn gives after the skill's `$<slug>`.
pub(in crate::execution) struct SetupSkill {
  pub skill: CodexChatSkill,
  pub words: String,
}

pub(in crate::execution) struct AppServerSetup {
  /// The turn's prompt; for a [`Self::skill`] Codex did not register, the message as kept, the skill's text in it.
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
  /// The servers the step chooses, Orchester's among them, by the names Codex gets them by.
  pub expected_servers: BTreeSet<String>,
  /// The servers Codex gets by another name than the project's (`CodexMcp::named_apart`): that name, then the
  /// project's, which the step's activity and approvals name them by.
  pub renamed: BTreeMap<String, String>,
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
  /// The session compacts the thread it resumes (`thread/compact/start`, ORC-309) in place of a turn of the prompt.
  pub compact: bool,
  /// The skill the turn loads as Codex's own; `None` for a step and a message without one.
  pub skill: Option<SetupSkill>,
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
