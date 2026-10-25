//! Whether an agent's event searched or read the code index it got (ORC-90), so that Usage can tell an invocation
//! that used the index from one that only got it ([`super::EditProgress::index_searches`]):
//!
//! - a call of Orchester's tools that read the index, `find_symbol`, `search_file`, `find_references` and `list_files`,
//!   that succeeded: Codex reports it as an MCP call of [`SERVER`], Claude Code as a command named
//!   `mcp__Orchester__<tool>`;
//! - a command or a read that names the index folder (`.orchester/index/`, either separator) or `$IDX`, the variable
//!   the internal `orchester-codebase-index` skill sets to it: a Codex `rg` command, Claude Code's `Bash`, `Read` or
//!   Orchester's `read_external_file`, a native agent's `read_file`. A command the CLI's sandbox declined did not run.
//!
//! The events of every thread of a Codex App Server connection are read alike, and Claude Code prints its
//! sub-agents' tool calls in its own stream, so a sub-agent's searches count for its invocation too (Codex's own
//! sub-agents are off for a step: `crate::connection::invocation::CODEX_SUB_AGENTS_OFF`). A Codex MCP call of
//! `read_external_file` or `search_external_file` is reported without its arguments and is not seen; nor is a search a
//! shell command makes through a variable of another name.

use crate::{
  connection::stream::{AgentEvent, CommandStatus},
  orchester_tools::{OrchesterTool, SERVER},
};

/// Orchester's tools whose answers come from the code index the agent got.
const INDEX_TOOLS: [OrchesterTool; 4] = [
  OrchesterTool::FindSymbol,
  OrchesterTool::SearchFile,
  OrchesterTool::FindReferences,
  OrchesterTool::ListFiles,
];
/// Where a repository's indexes are kept (`orchester_index`), with `/` separators and in lower case.
const INDEX_FOLDER: &str = ".orchester/index/";
/// The variable the index skill tells the agent to set to the index folder.
const SKILL_VARIABLE: &str = "$IDX";

/// Whether `event` is a search or read of the code index.
pub fn searches_code_index(event: &AgentEvent) -> bool {
  match event {
    AgentEvent::McpToolFinished {
      server, tool, status, ..
    } => {
      server == SERVER && *status == CommandStatus::Completed && OrchesterTool::from_name(tool).is_some_and(reads_index)
    }
    AgentEvent::CommandFinished { command, status, .. } => {
      let called = command
        .split_whitespace()
        .next()
        .and_then(OrchesterTool::from_claude_name);
      match called {
        Some(tool) => *status == CommandStatus::Completed && (reads_index(tool) || names_index(command)),
        None => *status != CommandStatus::Declined && names_index(command),
      }
    }
    _ => false,
  }
}

fn reads_index(tool: OrchesterTool) -> bool {
  INDEX_TOOLS.contains(&tool)
}

/// Whether `text`, a command line or a call's subject, names the index folder.
fn names_index(text: &str) -> bool {
  text.contains(SKILL_VARIABLE) || text.replace('\\', "/").to_lowercase().contains(INDEX_FOLDER)
}
