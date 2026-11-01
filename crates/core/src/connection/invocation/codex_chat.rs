//! How a turn of the project's Orchestrator chat (`crate::orchestrator`) starts Codex: App Server, as a step starts it
//! ([`super::CodexInvocation`], read-only in the project folder, no network, nothing writable), with every tool of
//! Codex's own that reads, runs or fetches anything turned off ([`CONFINEMENT`]):
//!
//! `codex app-server --stdio <isolation> -c projects={<folder>={trust_level="untrusted"}} [-c windows.sandbox=elevated]
//!  -c sandbox_workspace_write.writable_roots=[] -c project_doc_max_bytes=0 [-c skills.config=[…]]`
//! `<the chat's MCP server> <confinement>`, every skill of the folder in `skills.config` with `enabled=false`.
//!
//! The confinement does not rest on Codex's sandbox, whose `read-only` mode keeps writes out but not reads: Codex is
//! given no shell at all, so neither its sign-in in `CODEX_HOME`, nor other repositories, nor the runner's private
//! files can be read by a command, on Windows and macOS alike, and the person's own execpolicy rules, which only ever
//! match a shell command, have nothing to run. Checked with Codex 0.159.2 against a stand-in of the model's API, which
//! records the tools Codex offers (2026-10-25, Windows; the tools are built from these switches the same way on
//! macOS): App Server offered the model only `request_user_input` and `apply_patch`; a call of `exec_command`,
//! `shell` or `shell_command` was answered `unsupported call` and ran nothing, even in a `danger-full-access` sandbox;
//! `apply_patch` was refused by Codex's own approval check before any sandbox ran ("writing is blocked by read-only
//! sandbox; rejected by user approval settings"), as the runner's approval policy leaves nothing beyond the sandbox to
//! a person. What the agent reads of the project, it reads through the chat's git tools.
//!
//! - Its only MCP server is the chat's domain tools (`orchester`), their bearer token in the process environment only
//!   ([`super::CodexMcp`]); the user's own servers, plugins and apps are turned off as for a step.
//! - The repository's own `AGENTS.md`, `.codex` configuration and skills are left out, as for a step without its agent
//!   files: the folder is untrusted, `project_doc_max_bytes=0` keeps `AGENTS.md` out, and every skill of its
//!   `.agents/skills` and `.codex/skills`, which Codex loads even from an untrusted folder, is disabled by its paths
//!   ([`CodexChat::skills`]). The runner finds them as for a step and starts no turn when that search could miss one.
//! - The system prompt is App Server's `baseInstructions`, in place of Codex's own, on every turn: the runner gives it
//!   with the thread's start or resume (`orchester-runner`'s `chat::codex`).

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

use super::{CodexAgentFiles, CodexInvocation, CodexMcp, InvocationError, Platform};
use crate::{
  command::Command,
  permission::NetworkAccess,
  resource::{CommandEnvironment, RepositoryAccess},
};

/// The overrides that leave Codex no tool of its own that reads files, runs a program, looks at an image, searches the
/// web or works beside the turn, each a feature or setting of Codex 0.159.2. `shell_tool=false` is what takes its
/// shells away (`shell`, `shell_command`, `exec_command`, `write_stdin`): `codex features list` shows it off, and the
/// probe above saw none of them offered and each call refused. `unified_exec=false` is kept for a version that reads
/// it, but 0.159.2's `codex features list` still shows `unified_exec` on with it, so nothing rests on it. `view_image`
/// reads an image file outside any sandbox; `skill_mcp_dependency_install` would let a skill bring an MCP server into
/// the thread; the rest offer tools the chat has no use for. Each of these but `unified_exec` shows off in
/// `codex features list` with the overrides. Sub-agents, computer use and the user's own hooks are off for every Codex
/// invocation already ([`super::CODEX_SUB_AGENTS_OFF`], [`super::CODEX_ISOLATION`]). A turned-off tool is not offered
/// to the model, and a call of it is refused.
pub const CONFINEMENT: [&str; 9] = [
  "features.shell_tool=false",
  "features.unified_exec=false",
  "features.view_image=false",
  "features.skill_mcp_dependency_install=false",
  "features.goals=false",
  "features.memories=false",
  "features.browser_use=false",
  "features.image_generation=false",
  "web_search=\"disabled\"",
];

/// Everything that decides how one turn of the Orchestrator chat starts Codex App Server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexChat<'a> {
  pub executable: &'a Path,
  /// The project folder, the working directory.
  pub folder: &'a Path,
  pub platform: Platform,
  /// The chat's domain tools.
  pub mcp: &'a CodexMcp,
  /// Every skill of the project folder, each to be disabled: the paths of
  /// [`RepositorySkills::codex_paths`](crate::agent_files::skills::RepositorySkills::codex_paths). Codex loads a
  /// folder's skills even when the folder is untrusted, and the chat loads none.
  pub skills: &'a [PathBuf],
}

impl CodexChat<'_> {
  /// Arguments after the program, in order.
  pub fn args(&self) -> Result<Vec<String>, InvocationError> {
    let mut args = self.invocation().args()?;
    args.extend(confinement());
    Ok(args)
  }

  /// The App Server process: in the project folder, with the allowed part of `environment`, the variables the project
  /// adds (`project`) and the tools' token.
  pub fn command(
    &self,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
    project: &CommandEnvironment,
  ) -> Result<Command, InvocationError> {
    let command = self.invocation().command(environment, project)?;
    Ok(command.with_args(confinement()))
  }

  /// A read-only step's invocation in the project folder, which the confinement adds to.
  const fn invocation(&self) -> CodexInvocation<'_> {
    CodexInvocation {
      executable: self.executable,
      repository: self.folder,
      access: RepositoryAccess::ReadOnly,
      network: NetworkAccess::Off,
      writable_roots: &[],
      worktree: self.folder,
      platform: self.platform,
      mcp: Some(self.mcp),
      agent_files: CodexAgentFiles::LeftOut { skills: self.skills },
    }
  }
}

/// [`CONFINEMENT`] as Codex's `-c` overrides.
fn confinement() -> impl Iterator<Item = String> {
  CONFINEMENT
    .into_iter()
    .flat_map(|setting| ["-c".to_owned(), setting.to_owned()])
}
