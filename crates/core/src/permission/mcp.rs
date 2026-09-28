//! What each agent CLI enforces for the MCP servers a profile chooses
//! (`docs/design/orchester-workspace.md`, "MCP servers"). Checked against Codex CLI 0.139 and Claude
//! Code 2.1.89 on Windows on 2026-09-28, without a model turn (neither CLI was signed in for the
//! check), so what happens when an agent calls a server's tool is not verified.
//!
//! - Both CLIs load only the servers the invocation names: Codex leaves the user's own configuration out
//!   (`--ignore-user-config`), Claude Code every other configuration (`--strict-mcp-config`).
//! - Claude Code allows the chosen servers' tools with `--allowedTools mcp__<server>`; print mode denies any tool no
//!   rule allows. Codex runs with `-a never`: nobody is asked before a tool of a chosen server runs.
//! - Neither CLI's sandbox or permission settings reach inside a server: the server is a program of its own (or a
//!   service elsewhere) that works outside the worktree's limits and reaches the network whatever the Network setting
//!   says.
//! - Codex starts a server in the folder it is set to; Claude Code starts every server in the worktree.

use serde::{Deserialize, Serialize};

use crate::connection::AgentProvider;

/// How a CLI keeps the agent to the chosen servers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpLoading {
  /// `--ignore-user-config` and `-c mcp_servers.<server>…` overrides.
  CodexConfigOverrides,
  /// `--strict-mcp-config --mcp-config <file>`.
  ClaudeStrictConfig,
}

/// Whether a person approves a call of a chosen server's tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpCalls {
  /// `-a never`: calls run without anyone being asked.
  CodexNeverAsks,
  /// `--allowedTools mcp__<server>`: the chosen servers' tools are allowed, nothing else is.
  ClaudeAllowedTools,
}

/// What a CLI enforces for MCP servers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct McpSupport {
  pub loading: McpLoading,
  pub calls: McpCalls,
  /// The CLI starts a server in the folder it is set to; otherwise in the worktree.
  pub working_directory: bool,
}

pub const fn mcp_support(provider: AgentProvider) -> McpSupport {
  match provider {
    AgentProvider::CodexCli => McpSupport {
      loading: McpLoading::CodexConfigOverrides,
      calls: McpCalls::CodexNeverAsks,
      working_directory: true,
    },
    AgentProvider::ClaudeCodeCli => McpSupport {
      loading: McpLoading::ClaudeStrictConfig,
      calls: McpCalls::ClaudeAllowedTools,
      working_directory: false,
    },
  }
}
