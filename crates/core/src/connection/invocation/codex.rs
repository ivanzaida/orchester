//! How an agent step starts the Codex CLI (`.tmp/cli-probe/REPORT.md`, "Recommended argv"):
//!
//! `codex -a never exec --json --ignore-user-config --ignore-rules -m <model>
//!  [-c model_reasoning_effort=<e>] --sandbox <mode> [-c windows.sandbox=elevated]
//!  [-c sandbox_workspace_write.network_access=true] [-c sandbox_workspace_write.writable_roots=[<folders>]]
//!  [-c project_doc_max_bytes=0 [-c skills.config=[<skills>]]] [<MCP servers>] [--output-schema <file>] -C <worktree>
//! -`
//!
//! and continues a session after a pause or a limit with
//!
//! `codex -a never exec resume --json --ignore-user-config --ignore-rules -m <model>
//!  [-c model_reasoning_effort=<e>] -c sandbox_mode=<mode> [-c windows.sandbox=elevated]
//!  [-c sandbox_workspace_write.network_access=true] [-c sandbox_workspace_write.writable_roots=[<folders>]]
//!  [-c project_doc_max_bytes=0 [-c skills.config=[<skills>]]] [<MCP servers>] <session> -`
//!
//! in the worktree as the process's working directory.
//!
//! - `-a never` is a top-level flag: after `exec` it is an error. With it nothing ever waits for an approval; whatever
//!   the sandbox does not allow is declined and the agent is told.
//! - Permissions are enforced by Codex's own sandbox; bypass modes are never used. Read-only steps run in `read-only`;
//!   writing steps in `workspace-write`, which on Windows only confines writes with `windows.sandbox=elevated` (without
//!   it every command is declined).
//! - The mode and network access come from the step's enforced permissions
//!   ([`crate::permission::AgentPermissions::enforced_on`]); commands are always those the sandbox allows, since Codex
//!   cannot enforce a command list. Network access is a setting of the `workspace-write` sandbox only; the `read-only`
//!   sandbox has none.
//! - Extra writable folders ([`crate::permission::writable`]) are added to the `workspace-write` sandbox as a TOML
//!   array of basic strings. Checked 2026-09-28 with Codex CLI 0.139.0 on Windows 11 without a model turn: `codex exec`
//!   and `codex exec resume` both report them among the sandbox's writable roots (`exec resume` has no `--add-dir`, the
//!   equivalent of `exec`), the model is told them, and `codex sandbox` with `windows.sandbox=unelevated` lets a
//!   command write there and not beside them. A folder that does not exist is accepted and ignored, so the runner
//!   creates them first. The elevated sandbox was not run (it needs its one-time setup in the CLI's home); its log
//!   shows it grants write access per writable root.
//! - `--ignore-user-config` and `--ignore-rules` keep the user's own configuration (MCP servers, default model,
//!   execution rules) out of the run. The MCP servers the profile chooses are passed as configuration overrides
//!   ([`CodexMcp`]), their secrets through the process environment only.
//! - Repository agent files ([`crate::agent_files`]): while the project has them off, `AGENTS.md` is left out with
//!   `project_doc_max_bytes=0` and each skill of `<worktree>/.agents/skills` is disabled by the path of its `SKILL.md`
//!   (`{path="…",enabled=false}`), on new and resumed sessions alike.
//! - The prompt goes on standard input (`-`); free-form text in arguments would not survive the `.cmd` launcher on
//!   Windows.
//! - The environment is cleaned: see [`super::agent_environment`].
//! - `--output-schema` makes the final agent message JSON of the step result schema
//!   ([`crate::run::STEP_RESULT_SCHEMA`]). The probe verified it for new sessions only, so a continued session is
//!   started without it and its result is read from its exit.

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

use super::{
  CodexMcp, InvocationError, Platform, agent_command, agent_environment,
  mcp::{toml_array, toml_string},
  unicode_folders,
};
use crate::{
  command::Command,
  permission::NetworkAccess,
  resource::{CommandEnvironment, RepositoryAccess},
};

/// Everything that decides how one Codex invocation starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexInvocation<'a> {
  pub executable: &'a Path,
  pub model: &'a str,
  /// The profile's reasoning effort; `None` leaves Codex's default.
  pub reasoning_effort: Option<&'a str>,
  pub access: RepositoryAccess,
  /// Network access of a `workspace-write` sandbox.
  pub network: NetworkAccess,
  /// Folders outside the worktree a `workspace-write` sandbox also lets the agent write to, resolved
  /// ([`crate::permission::WritableFolders::resolve`]).
  pub writable_roots: &'a [PathBuf],
  pub worktree: &'a Path,
  pub platform: Platform,
  /// The session to continue (`codex exec resume`), for an invocation that repeats the
  /// unfinished part of a step; `None` starts a new session.
  pub resume: Option<&'a str>,
  /// The file holding the step result schema, for a new session.
  pub output_schema: Option<&'a Path>,
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
  /// Starts a bidirectional App Server with the normal Codex profile and its configured MCP servers.
  pub fn app_server_command(
    &self,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
    project: &CommandEnvironment,
  ) -> Result<Command, InvocationError> {
    unicode_folders(self.writable_roots)?;
    let mut args = vec!["app-server".to_owned(), "--stdio".to_owned()];
    if self.access == RepositoryAccess::Write && self.platform == Platform::Windows {
      args.extend(["-c".to_owned(), "windows.sandbox=elevated".to_owned()]);
    }
    if self.access == RepositoryAccess::Write {
      args.extend([
        "-c".to_owned(),
        format!(
          "sandbox_workspace_write.network_access={}",
          self.network == NetworkAccess::On
        ),
      ]);
    }
    if self.access == RepositoryAccess::Write {
      let roots: Vec<_> = self.writable_roots.iter().map(|root| root.to_string_lossy()).collect();
      args.extend([
        "-c".to_owned(),
        format!("sandbox_workspace_write.writable_roots={}", toml_array(&roots)),
      ]);
    }
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
      args.extend(mcp.args().iter().cloned());
    }
    let mut variables = agent_environment(environment);
    project.add_to(&mut variables);
    let command = Command::new(self.executable, args)
      .with_current_dir(self.worktree)
      .with_environment(variables)
      .with_interactive_stdin();
    let Some(mcp) = self.mcp else {
      return Ok(command);
    };
    Ok(mcp.secrets().iter().cloned().fold(command, Command::with_secret_env))
  }

  /// Arguments after the program, in order.
  pub fn args(&self) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["-a", "never", "exec"].into_iter().map(OsString::from).collect();
    if self.resume.is_some() {
      args.push("resume".into());
    }
    args.extend(
      ["--json", "--ignore-user-config", "--ignore-rules", "-m", self.model]
        .into_iter()
        .map(OsString::from),
    );
    if let Some(effort) = self.reasoning_effort {
      args.push("-c".into());
      args.push(format!("model_reasoning_effort={effort}").into());
    }
    let mode = match self.access {
      RepositoryAccess::ReadOnly => "read-only",
      RepositoryAccess::Write => "workspace-write",
    };
    // `exec resume` has no `--sandbox` or `-C`: the sandbox is set through its configuration key
    // and the working directory is the process's.
    if self.resume.is_some() {
      args.push("-c".into());
      args.push(format!("sandbox_mode={mode}").into());
    } else {
      args.push("--sandbox".into());
      args.push(mode.into());
    }
    if (self.access, self.platform) == (RepositoryAccess::Write, Platform::Windows) {
      args.push("-c".into());
      args.push("windows.sandbox=elevated".into());
    }
    if (self.access, self.network) == (RepositoryAccess::Write, NetworkAccess::On) {
      args.push("-c".into());
      args.push("sandbox_workspace_write.network_access=true".into());
    }
    if self.access == RepositoryAccess::Write && !self.writable_roots.is_empty() {
      let roots: Vec<_> = self.writable_roots.iter().map(|root| root.to_string_lossy()).collect();
      args.push("-c".into());
      args.push(format!("sandbox_workspace_write.writable_roots={}", toml_array(&roots)).into());
    }
    if let CodexAgentFiles::LeftOut { skills } = self.agent_files {
      args.push("-c".into());
      args.push("project_doc_max_bytes=0".into());
      if !skills.is_empty() {
        let disabled: Vec<String> = skills
          .iter()
          .map(|skill| format!("{{path={},enabled=false}}", toml_string(&skill.to_string_lossy())))
          .collect();
        args.push("-c".into());
        args.push(format!("skills.config=[{}]", disabled.join(",")).into());
      }
    }
    if let Some(mcp) = self.mcp {
      args.extend(mcp.args().iter().map(OsString::from));
    }
    match self.resume {
      Some(session) => args.push(session.into()),
      None => {
        if let Some(schema) = self.output_schema {
          args.push("--output-schema".into());
          args.push(schema.as_os_str().to_owned());
        }
        args.push("-C".into());
        args.push(self.worktree.as_os_str().to_owned());
      }
    }
    args.push("-".into());
    args
  }

  /// The command that runs the step: in the worktree, with `prompt` on standard input, the
  /// allowed part of `environment` and the variables the project adds (`project`).
  pub fn command(
    &self,
    prompt: String,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
    project: &CommandEnvironment,
  ) -> Result<Command, InvocationError> {
    unicode_folders(self.writable_roots)?;
    if let CodexAgentFiles::LeftOut { skills } = self.agent_files {
      unicode_folders(skills)?;
    }
    agent_command(
      self.executable,
      self.args(),
      self.worktree,
      prompt,
      (environment, project, self.mcp.map_or(&[][..], CodexMcp::secrets)),
    )
  }
}
