//! The folders an agent CLI keeps its own files of a run in, besides the worktree, which count as the run's
//! ([`super::RunRoots`]):
//!
//! - Claude Code: `<temp>/claude/<folder>`, where its background tasks write their output (`tasks/<id>.output`), and
//!   `<CLAUDE_CONFIG_DIR>/projects/<folder>`, its sessions and the tool results it keeps there, `<folder>` being the
//!   worktree with every character but a letter or digit as `-` (`H:\p\.orchester\w` is `H--p--orchester-w`; seen with
//!   Claude Code 2.1 on Windows, 2026-10-29). Not the rest of the configuration folder, which holds its sign-in.
//! - Codex: `<CODEX_HOME>/sessions`, where it writes each session's rollout. Not the rest of `CODEX_HOME` (`auth.json`,
//!   `config.toml`, its rules).
//! - A native agent: none; Orchester runs it and keeps its files itself.

use std::path::{Path, PathBuf};

use crate::connection::{AgentProvider, invocation::Platform};

/// Where an agent CLI keeps its files: the OS temporary folder its environment names, and its own configuration
/// folder (`CLAUDE_CONFIG_DIR`, else `~/.claude`; `CODEX_HOME`, else `~/.codex`).
#[derive(Clone, Copy, Debug)]
pub struct AgentHomes<'a> {
  pub temp: &'a Path,
  pub config: &'a Path,
}

/// The folders `provider`'s CLI keeps its own files of the run in `worktree` (as the CLI was started in it) in, as
/// `homes` place them, written in `platform`'s form.
pub fn agent_temp_folders(
  provider: AgentProvider,
  platform: Platform,
  worktree: &Path,
  homes: AgentHomes<'_>,
) -> Vec<PathBuf> {
  let separator = match platform {
    Platform::Windows => '\\',
    Platform::MacOs => '/',
  };
  let under = |home: &Path, names: &[&str]| {
    let mut path = home.to_string_lossy().trim_end_matches(['/', '\\']).to_owned();
    for name in names {
      path.push(separator);
      path.push_str(name);
    }
    PathBuf::from(path)
  };
  match provider {
    AgentProvider::ClaudeCodeCli => {
      let folder = claude_folder(worktree);
      vec![
        under(homes.temp, &["claude", &folder]),
        under(homes.config, &["projects", &folder]),
      ]
    }
    AgentProvider::CodexCli => vec![under(homes.config, &["sessions"])],
    AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => Vec::new(),
  }
}

/// The name Claude Code gives the folder `worktree` for: every character but an ASCII letter or digit as `-`.
fn claude_folder(worktree: &Path) -> String {
  worktree
    .to_string_lossy()
    .chars()
    .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
    .collect()
}
