//! Where a Cargo build writes (the person's decision of 2026-10-18, `docs/design/orchester-workspace.md`, "Auto
//! mode"): `cargo build`, `check`, `test`, `clippy`, `doc` and `bench` write their output into one target directory
//! and change no source Orchester can tell of (a stale `Cargo.lock` is still brought up to date, as any build does).
//! The target directory is the folder `--target-dir` names, else the one the shell's `CARGO_TARGET_DIR` (or
//! `CARGO_BUILD_TARGET_DIR`) names ([`CommandScope::with_cargo_target_dir`]), else `target` in the workspace's root.
//! That last one is told only for Cargo run in the worktree's root without `--manifest-path`, where the worktree's
//! `Cargo.toml` is a workspace's root (or no folder above the worktree holds a `Cargo.toml`) and no configuration file
//! Cargo reads there (`.cargo/config.toml` or `.cargo/config` in the worktree and each folder above it, `config.toml`
//! or `config` in a Cargo home) mentions a target directory. `--fix` and its companions change sources and `--open`
//! starts a browser: a build with them is none. A folder outside the worktree and its writable folders, one of them
//! itself, or one in `.git` or `.orchester/`, holds no build output. Whether the worktree's repository ignores the
//! folder is git's to tell ([`crate::run::review::HoldRules::build_output`]).

use std::{
  io,
  path::{Path, PathBuf},
};

use super::{CommandScope, part::CommandPart, paths::Place, place::Placing};

/// Cargo's subcommands that write only build output.
const SUBCOMMANDS: [&str; 9] = ["build", "b", "check", "c", "test", "t", "clippy", "doc", "bench"];

/// Their options that change sources, or start another program.
const REFUSED: [&str; 6] = [
  "--fix",
  "--allow-dirty",
  "--allow-staged",
  "--allow-no-vcs",
  "--broken-code",
  "--open",
];

/// The configuration files Cargo reads in a folder's `.cargo`, and in a Cargo home.
const CONFIG_FILES: [&str; 2] = ["config.toml", "config"];

/// A target directory a Cargo build writes its output into, resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildOutput {
  folder: PathBuf,
  in_worktree: Option<PathBuf>,
}

impl BuildOutput {
  pub fn folder(&self) -> &Path {
    &self.folder
  }

  /// The folder relative to the worktree, where the worktree's repository tells whether it ignores it; `None` in a
  /// writable folder outside the worktree.
  pub fn in_worktree(&self) -> Option<&Path> {
    self.in_worktree.as_deref()
  }
}

/// The words of a Cargo build that tell where its output goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BuildWords {
  /// The value of `--target-dir`, as written.
  target_dir: Option<String>,
  /// `--manifest-path` names the package, whose workspace may lie elsewhere than the folder Cargo runs in.
  manifest: bool,
}

impl BuildWords {
  /// The words of `cargo <sub> <rest>` when it writes only build output; `None` for another subcommand, a refused
  /// option anywhere, or `--target-dir` given twice or without a value.
  pub(super) fn of(sub: &str, rest: &[String]) -> Option<Self> {
    if !SUBCOMMANDS.contains(&sub) {
      return None;
    }
    let name = |arg: &str| arg.split_once('=').map_or(arg, |(name, _)| name).to_owned();
    if rest.iter().any(|arg| REFUSED.contains(&name(arg).as_str())) {
      return None;
    }
    // What follows `--` is the test binary's or the linter's, not Cargo's.
    let mut args = rest.iter().take_while(|arg| *arg != "--");
    let mut words = Self {
      target_dir: None,
      manifest: false,
    };
    while let Some(arg) = args.next() {
      match arg.split_once('=') {
        Some(("--target-dir", value)) => words.target(value.to_owned())?,
        None if arg == "--target-dir" => words.target(args.next()?.clone())?,
        Some(("--manifest-path", _)) => words.manifest = true,
        None if arg == "--manifest-path" => words.manifest = true,
        _ => {}
      }
    }
    Some(words)
  }

  /// Notes `--target-dir`'s `value`; `None` when it was given before.
  fn target(&mut self, value: String) -> Option<()> {
    self.target_dir.replace(value).is_none().then_some(())
  }
}

/// Notes where `part`, a Cargo build of `words`, writes its output: the one target directory every folder it may run
/// in gives, when that folder may hold build output.
pub(super) fn place(part: &mut CommandPart, words: &BuildWords, placing: Placing<'_>) {
  let scope = placing.scope;
  let Some(folders) = placing.folders else {
    return;
  };
  let targets: Option<Vec<(PathBuf, Place)>> = folders.iter().map(|folder| target(scope, folder, words)).collect();
  let Some(((folder, place), rest)) = targets.as_deref().and_then(<[_]>::split_first) else {
    return;
  };
  if rest.iter().any(|(other, _)| other != folder) || *place != Place::Granted {
    return;
  }
  if scope.is_root(folder) || scope.write_refused(folder, false) {
    return;
  }
  part.build_output = Some(BuildOutput {
    folder: folder.clone(),
    in_worktree: folder.strip_prefix(scope.worktree()).ok().map(Path::to_path_buf),
  });
}

/// The target directory a build of `words` run in `folder` writes to, resolved, and where it lies; `None` when it
/// cannot be told.
fn target(scope: &CommandScope, folder: &Path, words: &BuildWords) -> Option<(PathBuf, Place)> {
  let named = |word: &str| match scope.locate(folder, word)?.as_slice() {
    // Cargo expands neither `~` nor a glob.
    [only] if !word.starts_with('~') && !word.contains(['*', '?', '[']) => Some(only.clone()),
    _ => None,
  };
  if let Some(word) = &words.target_dir {
    return named(word);
  }
  if let Some(variable) = scope.cargo_target() {
    return named(&variable.to_string_lossy());
  }
  let worktree = scope.worktree();
  if words.manifest || folder != worktree || !workspace_root(worktree) || config_names_target(scope, folder) {
    return None;
  }
  named("target")
}

/// Whether Cargo takes the worktree's root for its workspace's root: its `Cargo.toml` declares a workspace, or no
/// folder above it holds a `Cargo.toml` whose workspace could take the worktree in.
fn workspace_root(worktree: &Path) -> bool {
  match std::fs::read_to_string(worktree.join("Cargo.toml")) {
    Ok(manifest) if manifest.lines().any(|line| line.trim() == "[workspace]") => true,
    Ok(_) => worktree
      .ancestors()
      .skip(1)
      .all(|folder| missing(&folder.join("Cargo.toml"))),
    Err(_) => false,
  }
}

/// Whether nothing is at `path`; a path that cannot be read may hold something.
fn missing(path: &Path) -> bool {
  matches!(std::fs::symlink_metadata(path), Err(error) if error.kind() == io::ErrorKind::NotFound)
}

/// Whether a configuration file Cargo reads when it runs in `folder` mentions a target directory, or cannot be read.
fn config_names_target(scope: &CommandScope, folder: &Path) -> bool {
  let in_folders = folder
    .ancestors()
    .flat_map(|above| CONFIG_FILES.map(|name| above.join(".cargo").join(name)));
  let in_homes = scope
    .cargo_homes()
    .iter()
    .flat_map(|home| CONFIG_FILES.map(|name| home.join(name)));
  in_folders
    .chain(in_homes)
    .any(|file| match std::fs::read_to_string(&file) {
      Ok(text) => text.contains("target-dir"),
      Err(error) => error.kind() != io::ErrorKind::NotFound,
    })
}
