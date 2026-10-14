//! Where Rust's toolchains keep the sources of dependencies (the user's decision of 2026-10-14): the crates
//! Cargo unpacks from its registries (`<cargo home>/registry/src`) and checks out from git
//! (`<cargo home>/git/checkouts`), and the toolchains' library sources (`<rustup home>/toolchains/*/lib/rustlib/src`).
//! They hold code published for everyone, so a part that only reads reads them as it reads the worktree; a write
//! there is still a write outside it. The rest of a Cargo or rustup home (`credentials.toml`, `config.toml`, the
//! registry's index and cache, `settings.toml`) is a credential location wherever the home lies. The homes are
//! `.cargo` and `.rustup` in each home folder, and `CARGO_HOME` and `RUSTUP_HOME` where they name others.

use std::path::{Path, PathBuf};

/// The dependency sources in a Cargo home, by the names of their folders.
const CARGO_SOURCES: [&[&str]; 2] = [&["registry", "src"], &["git", "checkouts"]];
/// The dependency sources in a rustup home; `*` is any one name.
const RUSTUP_SOURCES: [&[&str]; 1] = [&["toolchains", "*", "lib", "rustlib", "src"]];

/// The Cargo and rustup homes, resolved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Toolchains {
  cargo: Vec<PathBuf>,
  rustup: Vec<PathBuf>,
}

impl Toolchains {
  /// Adds a Cargo home and a rustup home, each resolved.
  pub(super) fn add(&mut self, cargo: Option<PathBuf>, rustup: Option<PathBuf>) {
    for (homes, home) in [(&mut self.cargo, cargo), (&mut self.rustup, rustup)] {
      if let Some(home) = home.filter(|home| !homes.contains(home)) {
        homes.push(home);
      }
    }
  }

  /// Where `path` (resolved) lies in the toolchains' homes: `Some(true)` in their dependency sources,
  /// `Some(false)` elsewhere in a home, `None` in none of them.
  pub(super) fn sources(&self, path: &Path) -> Option<bool> {
    let verdicts: Vec<bool> = within(&self.cargo, path, &CARGO_SOURCES)
      .chain(within(&self.rustup, path, &RUSTUP_SOURCES))
      .collect();
    (!verdicts.is_empty()).then(|| verdicts.iter().all(|sources| *sources))
  }
}

/// For each of `homes` that holds `path`, whether `path` lies in one of the `sources` folders there.
fn within<'a>(homes: &'a [PathBuf], path: &'a Path, sources: &'a [&[&str]]) -> impl Iterator<Item = bool> + 'a {
  homes.iter().filter_map(move |home| {
    let names: Vec<String> = path
      .strip_prefix(home)
      .ok()?
      .components()
      .map(|part| part.as_os_str().to_string_lossy().into_owned())
      .collect();
    Some(sources.iter().any(|folder| {
      names.len() >= folder.len()
        && folder
          .iter()
          .zip(&names)
          .all(|(pattern, name)| *pattern == "*" || *pattern == name.as_str())
    }))
  })
}
