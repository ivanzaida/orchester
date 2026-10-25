//! How an agent step starts the Codex CLI: always as a bidirectional App Server, so that a request
//! for a person's decision can reach "Needs you" (`docs/interactive-tool-approvals.md`):
//!
//! `codex app-server --stdio <isolation> <sub-agents off>
//!  -c projects={<worktree>={trust_level="untrusted"},<repository>=…}
//!  [-c windows.sandbox=elevated] [-c sandbox_workspace_write.network_access=<on>]
//!  -c sandbox_workspace_write.writable_roots=[<worktree>, <folders>] [-c project_doc_max_bytes=0
//!  [-c skills.config=[<skills>]]] [<MCP servers>]`
//!
//! in the worktree as the process's working directory, with standard input kept open for the runner's
//! JSON-RPC requests. The runner starts or resumes the thread with the model, the sandbox, the approval
//! policy and the reviewer, checks what Codex activated, and starts the turn with the prompt, the effort
//! and, for a new session, the step result schema (`orchester-runner`'s `step::app_server`).
//!
//! - Permissions are enforced by Codex's own sandbox; bypass modes are never used. Read-only steps run in `read-only`,
//!   writing steps in `workspace-write`. Both modes need `windows.sandbox=elevated` on Windows; without a Windows
//!   sandbox even read-only shell commands are declined.
//! - Network access is a setting of the `workspace-write` sandbox only; the `read-only` sandbox has none. A command
//!   that needs more than the sandbox (network while it is off, a write outside the writable roots, an escalation the
//!   model requests) asks a person through the granular approval policy's `sandbox_approval`
//!   ([`crate::permission::OutsideCommands::SandboxApproval`]); approved, Codex runs that one command outside the
//!   sandbox.
//! - Extra writable folders ([`crate::permission::writable`]) are added to the `workspace-write` sandbox as a TOML
//!   array of basic strings, with the worktree, which Codex also reports as the local environment's workspace root. A
//!   folder that does not exist is accepted and ignored, so the runner creates them first.
//! - App Server has no `--ignore-user-config` or `--ignore-rules`, so it loads the user's own configuration. What of it
//!   would reach the agent is turned off ([`ISOLATION`]): the `notify` program, plugins (with their skills and MCP
//!   servers), apps, and the permission requests that could stay granted for later commands. The user's own MCP servers
//!   are turned off for the thread by name, which the runner reads with `config/read` first
//!   (`crate::connection::app_server_mcp`). The user's own execpolicy `.rules` stay: they are the user's choice, and
//!   the enforcement panel says that a command an `allow` rule there matches runs outside the sandbox without asking.
//! - The repository's own `.codex` folder (configuration, hooks, execpolicy rules, MCP servers) is never loaded,
//!   whatever the user's configuration trusts: the worktree and its repository are marked untrusted
//!   ([`CodexInvocation::untrusted_projects`]), and the runner checks with `config/read` that Codex loaded no project
//!   layer (`crate::connection::app_server_config`). The runner checks the sandbox, approval policy, reviewer, network,
//!   writable roots and model Codex activated against the step's permissions before the turn starts. The MCP servers
//!   the profile chooses are passed as configuration overrides ([`CodexMcp`]), their secrets through the process
//!   environment only.
//! - Orchester's own tools ([`crate::orchester_tools`]) come from an MCP server of the same overrides,
//!   `mcp_servers.Orchester`: the runner's relay ([`super::CodexOrchester`]), its token in the process environment
//!   only. Codex's own sub-agents are turned off ([`SUB_AGENTS_OFF`]): they would not get these tools.
//! - Repository agent files ([`crate::agent_files`]): while the project has them off, `AGENTS.md` is left out with
//!   `project_doc_max_bytes=0` and each skill of `<worktree>/.agents/skills` is disabled by the path of its `SKILL.md`
//!   (`{path="…",enabled=false}`), on new and resumed sessions alike.
//! - The environment is cleaned: see [`super::agent_environment`].

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

use super::{
  CodexMcp, InvocationError, Platform, agent_environment,
  mcp::{toml_array, toml_string},
  unicode_folders,
};
use crate::{
  command::Command,
  permission::NetworkAccess,
  resource::{CommandEnvironment, RepositoryAccess},
};

/// Configuration overrides that keep the user's own Codex setup from the agent. Checked against Codex
/// 0.159.2 with `config/read` and `mcpServerStatus/list`, without a model turn (2026-10-01): `notify`
/// becomes empty (an array replaces the user's), and the plugin servers (`code-review`, `cua_repl`)
/// and `codex_app` are gone from the thread's MCP inventory. A table such as `mcp_servers` merges
/// with the user's rather than replacing it, so servers are turned off one by one instead.
pub const ISOLATION: [&str; 5] = [
  "notify=[]",
  "features.plugins=false",
  "features.apps=false",
  "features.exec_permission_approvals=false",
  "features.request_permissions_tool=false",
];

/// Codex's own sub-agents (`multi_agent`, on by default in 0.159.2, and `multi_agent_v2`), turned off for a step:
/// a sub-agent's tool list had none of the step's MCP tools, Orchester's included (`tools.mcp__Orchester__read_skill
/// is not a function`, issue #56), so the step's own agent does the work with them. `codex features list` with these
/// overrides shows both off (0.159.2, 2026-10-25). The step's prompt says so ([`SUB_AGENTS_PROMPT`]).
pub const SUB_AGENTS_OFF: [&str; 2] = ["features.multi_agent=false", "features.multi_agent_v2=false"];

/// What a Codex step's prompt says about its sub-agents being off ([`SUB_AGENTS_OFF`]).
pub const SUB_AGENTS_PROMPT: &str = "Codex's sub-agents are turned off for this step, because they would not get \
  Orchester's tools: do the work yourself, with the tools above.";

/// Everything that decides how one Codex App Server process starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexInvocation<'a> {
  pub executable: &'a Path,
  /// The repository the worktree belongs to.
  pub repository: &'a Path,
  pub access: RepositoryAccess,
  /// Network access of a `workspace-write` sandbox.
  pub network: NetworkAccess,
  /// Folders outside the worktree a `workspace-write` sandbox also lets the agent write to, resolved
  /// ([`crate::permission::WritableFolders::resolve`]).
  pub writable_roots: &'a [PathBuf],
  pub worktree: &'a Path,
  pub platform: Platform,
  /// The MCP servers the invocation gives the agent, if any.
  pub mcp: Option<&'a CodexMcp>,
  pub agent_files: CodexAgentFiles<'a>,
}

/// Whether Codex loads the repository's own agent files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodexAgentFiles<'a> {
  /// `AGENTS.md` and the skills of `.agents/skills`, as Codex does by default.
  Loaded,
  /// Neither: `skills` are the `SKILL.md` files of `<worktree>/.agents/skills` found when the agent
  /// starts, each disabled.
  LeftOut { skills: &'a [PathBuf] },
}

impl CodexInvocation<'_> {
  /// Arguments after the program, in order.
  pub fn args(&self) -> Result<Vec<String>, InvocationError> {
    unicode_folders(self.writable_roots)?;
    let mut args = vec!["app-server".to_owned(), "--stdio".to_owned()];
    for setting in ISOLATION.into_iter().chain(SUB_AGENTS_OFF) {
      args.extend(["-c".to_owned(), setting.to_owned()]);
    }
    args.extend(["-c".to_owned(), self.untrusted_projects()]);
    if self.platform == Platform::Windows {
      args.extend(["-c".to_owned(), "windows.sandbox=elevated".to_owned()]);
    }
    let writes = self.access == RepositoryAccess::Write;
    if writes {
      args.extend([
        "-c".to_owned(),
        format!(
          "sandbox_workspace_write.network_access={}",
          self.network == NetworkAccess::On
        ),
      ]);
    }
    let roots: Vec<_> = if writes {
      std::iter::once(self.worktree)
        .chain(self.writable_roots.iter().map(PathBuf::as_path))
        .map(Path::to_string_lossy)
        .collect()
    } else {
      Vec::new()
    };
    args.extend([
      "-c".to_owned(),
      format!("sandbox_workspace_write.writable_roots={}", toml_array(&roots)),
    ]);
    if let CodexAgentFiles::LeftOut { skills } = self.agent_files {
      unicode_folders(skills)?;
      args.extend(["-c".to_owned(), "project_doc_max_bytes=0".to_owned()]);
      if !skills.is_empty() {
        let disabled: Vec<String> = skills
          .iter()
          .map(|skill| format!("{{path={},enabled=false}}", toml_string(&skill.to_string_lossy())))
          .collect();
        args.extend(["-c".to_owned(), format!("skills.config=[{}]", disabled.join(","))]);
      }
    }
    if let Some(mcp) = self.mcp {
      args.extend(mcp.args());
    }
    Ok(args)
  }

  /// The worktree and its repository marked untrusted, so that Codex loads no `.codex` folder of
  /// theirs, even where the user's configuration trusts the repository. Codex 0.159.2 looks trust up
  /// by the folder it works in, then by the repository a worktree belongs to, not by other parent
  /// folders; a key must be written as Codex writes the folder ([`trust_key`]). Checked with
  /// `config/read` and the execpolicy load log, without a model turn (2026-10-01). An inline table
  /// merges with the user's own `projects`; a dotted key would split at the dot of `.orchester`.
  fn untrusted_projects(&self) -> String {
    let mut keys = vec![
      trust_key(self.worktree, self.platform),
      trust_key(self.repository, self.platform),
    ];
    // A key twice in one inline table would stop Codex from starting.
    keys.dedup();
    let entries: Vec<String> = keys
      .iter()
      .map(|key| format!("{}={{trust_level=\"untrusted\"}}", toml_string(key)))
      .collect();
    format!("projects={{{}}}", entries.join(","))
  }

  /// The App Server process: in the worktree, with the allowed part of `environment`, the variables
  /// the project adds (`project`) and the MCP servers' secrets.
  pub fn command(
    &self,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
    project: &CommandEnvironment,
  ) -> Result<Command, InvocationError> {
    let args = self.args()?;
    let mut variables = agent_environment(environment);
    project.add_to(&mut variables);
    let command = Command::new(self.executable, args)
      .with_current_dir(self.worktree)
      .with_environment(variables)
      .with_interactive_stdin();
    let secrets = self.mcp.map_or(&[][..], CodexMcp::secrets);
    Ok(secrets.iter().cloned().fold(command, Command::with_secret_env))
  }
}

/// `folder` as Codex names a project. On Windows Codex matches `\` separators only, without the
/// `\\?\` prefix of a canonical path, and with either case of the drive letter; its own messages
/// write it lower-case.
fn trust_key(folder: &Path, platform: Platform) -> String {
  let text = folder.to_string_lossy();
  if platform == Platform::MacOs {
    return text.into_owned();
  }
  let text = text.replace('/', r"\");
  let text = if let Some(share) = text.strip_prefix(r"\\?\UNC\") {
    format!(r"\\{share}")
  } else {
    text.strip_prefix(r"\\?\").map_or_else(|| text.clone(), str::to_owned)
  };
  let mut chars = text.chars();
  match (chars.next(), chars.next()) {
    (Some(drive), Some(':')) if drive.is_ascii_alphabetic() => format!("{}{}", drive.to_ascii_lowercase(), &text[1..]),
    _ => text,
  }
}
