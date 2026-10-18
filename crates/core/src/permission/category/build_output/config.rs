//! What Cargo reads besides the command when it builds in a folder: the workspace's root its manifests name, and its
//! configuration files (`.cargo/config.toml` or `.cargo/config` in the folder and each folder above it, `config.toml`
//! or `config` in each Cargo home). They are read as text, not parsed: what may change where a build writes or what
//! stands in for a subcommand is looked for by its words, and a file that cannot be read may say anything.

use std::{io, path::Path};

use super::super::CommandScope;

/// The configuration files Cargo reads in a folder's `.cargo`, and in a Cargo home.
const CONFIG_FILES: [&str; 2] = ["config.toml", "config"];

/// Whether Cargo takes the worktree's root for its workspace's root: its `Cargo.toml` declares a workspace, or no
/// folder above it holds a `Cargo.toml` whose workspace could take the worktree in.
pub(super) fn workspace_root(worktree: &Path) -> bool {
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

/// Whether a configuration file Cargo reads when it runs in `folder` holds any of `words`, or cannot be read, or
/// which files Cargo reads is not known.
pub(super) fn config_mentions(scope: &CommandScope, folder: &Path, words: &[&str]) -> bool {
  if scope.cargo().config_unknown {
    return true;
  }
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
      Ok(text) => words.iter().any(|word| text.contains(word)),
      Err(error) => error.kind() != io::ErrorKind::NotFound,
    })
}
