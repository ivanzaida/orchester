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

/// The variable naming the user's `%LOCALAPPDATA%`, where winget installs (Windows).
pub const LOCAL_APP_DATA: &str = "LOCALAPPDATA";
/// How the folders of winget's ripgrep packages are named at the start.
const WINGET_PACKAGE: &str = "BurntSushi.ripgrep";
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
  /// Where winget installs it, which a `PATH` read before the install does not have yet.
  Winget,
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

/// The first ripgrep on `search_path`, else one winget installed for the user whose
/// `%LOCALAPPDATA%` is `local_app_data` (Windows), else one bundled with the agent CLIs whose
/// executables are `clis`, in their order. winget adds the folder it installs into to the user's
/// `PATH`, which a program started before the install does not have yet.
pub fn locate(search_path: Option<&OsStr>, local_app_data: Option<&OsStr>, clis: &[&Path]) -> Option<Found> {
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
  if let Some(path) = winget_folder(local_app_data).as_deref().and_then(winget_installed) {
    return Some(Found {
      path,
      source: Source::Winget,
    });
  }
  clis.iter().find_map(|cli| bundled(cli))
}

/// The ripgrep winget installed under its folder `winget`: linked into `Links`, or in the newest
/// version folder of its package under `Packages` (`BurntSushi.ripgrep.MSVC_<source>\ripgrep-<version>-<target>`).
/// winget adds either folder to the user's `PATH`, which Orchester read when it started.
fn winget_installed(winget: &Path) -> Option<PathBuf> {
  let linked = winget.join("Links").join(PROGRAM);
  if linked.is_file() {
    return Some(linked);
  }
  // No Packages folder, or one that cannot be read, holds no ripgrep to use; the other places are
  // looked in next.
  let packages = fs::read_dir(winget.join("Packages")).ok()?;
  packages
    .filter_map(Result::ok)
    .filter(|package| package.file_name().to_string_lossy().starts_with(WINGET_PACKAGE))
    .filter_map(|package| fs::read_dir(package.path()).ok())
    .flat_map(|versions| versions.filter_map(Result::ok))
    .map(|version| {
      (
        version_of(&version.file_name().to_string_lossy()),
        version.path().join(PROGRAM),
      )
    })
    .filter(|(_, program)| program.is_file())
    .max()
    .map(|(_, program)| program)
}

/// The version numbers in a winget version folder's name (`ripgrep-15.1.0-x86_64-pc-windows-msvc`
/// is `[15, 1, 0]`), compared as numbers; none for a name without them.
fn version_of(folder: &str) -> Vec<u64> {
  folder
    .split('-')
    .nth(1)
    .map(|version| version.split('.').map_while(|part| part.parse().ok()).collect())
    .unwrap_or_default()
}

/// winget's folder under `%LOCALAPPDATA%`.
#[cfg(windows)]
fn winget_folder(local_app_data: Option<&OsStr>) -> Option<PathBuf> {
  Some(Path::new(local_app_data?).join("Microsoft").join("WinGet"))
}

/// There is no winget; Homebrew installs into folders the search path has (`search_path`).
#[cfg(not(windows))]
fn winget_folder(_local_app_data: Option<&OsStr>) -> Option<PathBuf> {
  None
}

/// The ripgrep bundled with the agent CLI whose executable is `cli`, if its install has one.
fn bundled(cli: &Path) -> Option<Found> {
  let launch = launch::resolve(cli);
  let program = match launch.via() {
    Via::NodeScript { script } => script.clone(),
    Via::LauncherProgram => launch.program().to_path_buf(),
    Via::Direct | Via::CommandScript => cli.to_path_buf(),
  };
  // npm links a global CLI's command to the file in its package (macOS), or the file is named by
  // the launcher (Windows): the package is one of the file's folders. A program whose
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
