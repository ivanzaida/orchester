//! ripgrep (`rg`) for agents, which the code index skill searches with (`docs/design/orchester-workspace.md`, "Code
//! index"). It is looked up on the `PATH` the agent starts with, then among the files of the agent CLIs that bundle a
//! copy: the Codex CLI's npm package (`@openai/codex-<platform>/vendor/<target>/codex-path/rg`) and Claude Code's
//! (`@anthropic-ai/claude-code/vendor/ripgrep/<arch>-<platform>/rg`). A native Claude Code build keeps its ripgrep
//! inside its own executable, where nothing else can run it.
//!
//! What is found is copied into the project's `.orchester/tools/rg-<hash>/`, outside every worktree, and that folder
//! goes first on the agent's `PATH`: Codex's Windows sandbox runs agents as a user that cannot run programs from
//! another user's profile (a winget install under `%LOCALAPPDATA%`), but can from the repository. A copy is named by a
//! hash of its content, so a new ripgrep gets a folder of its own and an agent never sees a copy change under it.
//! Installing ripgrep is [`install`], run only when a person asks for it.

mod copy;
pub mod install;

use std::{
  ffi::OsStr,
  fs, io,
  path::{Path, PathBuf},
};

use thiserror::Error;

pub use self::copy::provide;
use crate::launch::{self, Via};

/// The file name of ripgrep's program.
pub const PROGRAM: &str = if cfg!(windows) { "rg.exe" } else { "rg" };
/// The target folder of the Codex CLI's platform package.
const CODEX_TARGET: &str = if cfg!(all(windows, target_arch = "aarch64")) {
  "aarch64-pc-windows-msvc"
} else if cfg!(windows) {
  "x86_64-pc-windows-msvc"
} else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
  "aarch64-apple-darwin"
} else if cfg!(target_os = "macos") {
  "x86_64-apple-darwin"
} else if cfg!(target_arch = "aarch64") {
  "aarch64-unknown-linux-musl"
} else {
  "x86_64-unknown-linux-musl"
};
/// Claude Code's folder for this platform under `vendor/ripgrep`.
const CLAUDE_PLATFORM: &str = if cfg!(all(windows, target_arch = "aarch64")) {
  "arm64-win32"
} else if cfg!(windows) {
  "x64-win32"
} else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
  "arm64-darwin"
} else if cfg!(target_os = "macos") {
  "x64-darwin"
} else if cfg!(target_arch = "aarch64") {
  "arm64-linux"
} else {
  "x64-linux"
};

/// Where a ripgrep was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
  OnPath,
  CodexCli,
  ClaudeCode,
}

/// A ripgrep program and where it was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
  pub path: PathBuf,
  pub source: Source,
}

#[derive(Debug, Error)]
pub enum ProvideError {
  #[error("cannot read {path}: {source}")]
  Read { path: PathBuf, source: io::Error },
  #[error("cannot copy ripgrep to {path}: {source}")]
  Copy { path: PathBuf, source: io::Error },
}

/// The first ripgrep on `search_path`, else one bundled with the agent CLIs whose executables are
/// `clis`, in their order.
pub fn locate(search_path: Option<&OsStr>, clis: &[&Path]) -> Option<Found> {
  let on_path = search_path.and_then(|path| {
    std::env::split_paths(path)
      .filter(|folder| !folder.as_os_str().is_empty())
      .map(|folder| folder.join(PROGRAM))
      .find(|candidate| candidate.is_file())
  });
  if let Some(path) = on_path {
    return Some(Found {
      path,
      source: Source::OnPath,
    });
  }
  clis.iter().find_map(|cli| bundled(cli))
}

/// The ripgrep bundled with the agent CLI whose executable is `cli`, if its install has one.
fn bundled(cli: &Path) -> Option<Found> {
  let launch = launch::resolve(cli);
  let program = match launch.via() {
    Via::NodeScript { script } => script.clone(),
    Via::Direct | Via::CommandScript => cli.to_path_buf(),
  };
  // npm links a global CLI's command to the script in its package (macOS), or the script is
  // named by the launcher (Windows): the package is one of the script's folders. A program whose
  // links cannot be followed is looked at where it was named.
  let program = fs::canonicalize(&program).unwrap_or(program);
  program.ancestors().find_map(|folder| {
    let name = folder.file_name()?.to_str()?;
    let scope = folder.parent()?;
    match (scope.file_name()?.to_str()?, name) {
      ("@openai", "codex") => codex_ripgrep(folder, scope),
      ("@anthropic-ai", "claude-code") => {
        let path = folder
          .join("vendor")
          .join("ripgrep")
          .join(CLAUDE_PLATFORM)
          .join(PROGRAM);
        path.is_file().then_some(Found {
          path,
          source: Source::ClaudeCode,
        })
      }
      _ => None,
    }
  })
}

/// The ripgrep of the Codex CLI's package `package` in the npm scope folder `scope`: in its platform
/// package, installed inside it or beside it.
fn codex_ripgrep(package: &Path, scope: &Path) -> Option<Found> {
  let scopes = [package.join("node_modules").join("@openai"), scope.to_path_buf()];
  let mut packages: Vec<PathBuf> = vec![package.to_path_buf()];
  for scope in &scopes {
    let Ok(entries) = fs::read_dir(scope) else {
      continue;
    };
    let mut platform: Vec<PathBuf> = entries
      .filter_map(|entry| entry.ok().map(|entry| entry.path()))
      .filter(|path| {
        path
          .file_name()
          .and_then(OsStr::to_str)
          .is_some_and(|name| name.starts_with("codex-"))
      })
      .collect();
    platform.sort();
    packages.extend(platform);
  }
  packages.iter().find_map(|package| {
    ["codex-path", "path"].iter().find_map(|folder| {
      let path = package.join("vendor").join(CODEX_TARGET).join(folder).join(PROGRAM);
      path.is_file().then_some(Found {
        path,
        source: Source::CodexCli,
      })
    })
  })
}
