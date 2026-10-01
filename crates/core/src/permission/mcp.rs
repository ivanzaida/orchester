//! What each agent CLI enforces for the MCP servers a profile chooses
//! (`docs/design/orchester-workspace.md`, "MCP servers"). Checked against Codex CLI 0.159 and Claude
//! Code 2.1.89 on Windows. Claude Code was not signed in, so its `mcp__<server>` allow rule has not
//! been verified by a model turn.
//!
//! - Codex runs as App Server (`crate::connection::invocation::CodexInvocation`), which loads the user's own
//!   configuration and its global servers besides the chosen ones (`-c mcp_servers.<server>…` overrides). A tool call
//!   that asks for approval reaches a person when the step asks before MCP actions, and is declined otherwise (the
//!   approval policy's `mcp_elicitations`; `docs/interactive-tool-approvals.md`).
//! - Claude Code loads only the servers the invocation names (`--strict-mcp-config`) and allows their tools with
//!   `--allowedTools mcp__<server>`; print mode denies any tool no rule allows.
//! - Neither CLI's sandbox or permission settings reach inside a server: the server is a program of its own (or a
//!   service elsewhere) that works outside the worktree's limits and reaches the network whatever the Network setting
//!   says.

use crate::connection::AgentProvider;

/// Whether `provider` starts a server in the folder it is set to: Codex does, Claude Code starts
/// every server in the worktree, and a native agent starts none in this version.
pub const fn starts_in_its_folder(provider: AgentProvider) -> bool {
  match provider {
    AgentProvider::CodexCli => true,
    AgentProvider::ClaudeCodeCli | AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => false,
  }
}
