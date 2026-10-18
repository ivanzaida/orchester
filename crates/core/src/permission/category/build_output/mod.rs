//! Where a Cargo build writes (the person's decision of 2026-10-18, `docs/design/orchester-workspace.md`, "Auto
//! mode"): `cargo build`, `check`, `test`, `clippy`, `doc` and `bench` write their output into one target directory
//! and change no source Orchester can tell of (a stale `Cargo.lock` is still brought up to date, as any build does),
//! as long as only Cargo's own names of them are used and what follows `--` writes nothing ([`words`]). The target
//! directory is the folder `--target-dir` names, else the one the agent's environment names
//! ([`CommandScope::with_cargo_target_dir`]), else `target` in the workspace's root. That last one is told only for
//! Cargo run in the worktree's root without `--manifest-path`, where the worktree's `Cargo.toml` is a workspace's
//! root (or no folder above the worktree holds a `Cargo.toml`) and no configuration file Cargo reads there may move
//! it ([`config`]). `clippy` is no built-in subcommand, so an alias may stand in its place: it is a build only where
//! no configuration file names an alias. `--fix` and its companions change sources and `--open` starts a browser: a
//! build with them is none. A folder outside the worktree and its writable folders, one of them itself, or one in
//! `.git` or `.orchester/`, holds no build output. Whether the worktree's repository ignores the folder is git's to
//! tell ([`crate::run::review::HoldRules::build_output`]).

mod config;
mod environment;
mod words;

use std::path::{Path, PathBuf};

use self::config::{config_mentions, workspace_root};
pub use self::environment::build_variables;
pub(super) use self::{
  environment::{CargoEnvironment, variable},
  words::BuildWords,
};
use super::{CommandScope, part::CommandPart, paths::Place, place::Placing};

/// What in a configuration file may move the target directory: the setting, a key spelled with an escape, or another
/// file it includes.
const MOVES_TARGET: [&str; 4] = ["target-dir", "\\u", "\\U", "include"];
/// What in a configuration file may stand in place of `clippy`: an alias, a key spelled with an escape, or another
/// file it includes.
const MAY_ALIAS: [&str; 4] = ["alias", "\\u", "\\U", "include"];

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

/// Notes where `part`, a Cargo build of `words`, writes its output: the one target directory every folder it may run
/// in gives, when that folder may hold build output.
pub(super) fn place(part: &mut CommandPart, words: &BuildWords, placing: Placing<'_>) {
  let scope = placing.scope;
  let Some(folders) = placing.folders else {
    return;
  };
  let aliased = scope.cargo().may_alias(&words.subcommand, !words.clippy)
    || (words.clippy && folders.iter().any(|folder| config_mentions(scope, folder, &MAY_ALIAS)));
  if aliased {
    return;
  }
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
  if let Some(variable) = &scope.cargo().target {
    return named(&variable.to_string_lossy());
  }
  let worktree = scope.worktree();
  if words.manifest || folder != worktree || !workspace_root(worktree) || config_mentions(scope, folder, &MOVES_TARGET)
  {
    return None;
  }
  named("target")
}
