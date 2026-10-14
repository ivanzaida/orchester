//! Where Rust's toolchains keep the sources of dependencies (the user's decision of 2026-10-14): the crates
//! Cargo unpacks from its registries (`<cargo home>/registry/src`) and checks out from git
//! (`<cargo home>/git/checkouts`), and the toolchains' library sources (`<rustup home>/toolchains/*/lib/rustlib/src`).
//! They hold code published for everyone, but code nobody here wrote: a link in it may lead anywhere, a
//! `.cargo/config.toml` or a `.git/config` in it may name a program. So only [`SOURCE_READERS`] read them as they
//! read the worktree, and only the files and folders the line names or the folder they run in; anything else
//! there reads outside the worktree, a program that loads settings from the folder it runs in is Unknown there
//! ([`super::programs::Judged::folder_code`]), and a write there is a write outside the worktree. A `.git` folder
//! in them, and the rest of a Cargo or rustup home (`credentials.toml`, `config.toml`, the registry's index and
//! cache, `settings.toml`), is a credential location wherever the home lies. The homes are `.cargo` and `.rustup`
//! in each home folder, and `CARGO_HOME` and `RUSTUP_HOME` where they name others.

use std::path::{Path, PathBuf};

use super::{CommandCategory, part::CommandPart};

/// The dependency sources in a Cargo home, by the names of their folders.
const CARGO_SOURCES: [&[&str]; 2] = [&["registry", "src"], &["git", "checkouts"]];
/// The dependency sources in a rustup home; `*` is any one name.
const RUSTUP_SOURCES: [&[&str]; 1] = [&["toolchains", "*", "lib", "rustlib", "src"]];

/// The programs that read only the files they are given, and walk a folder without following the links in it
/// (`grep -r`, `rg`, `ls -R`; their options that follow links are refused by the known-safe tables and
/// [`super::programs`]), and load nothing from the folder they run in.
const SOURCE_READERS: [&str; 21] = [
  "cat",
  "head",
  "tail",
  "wc",
  "nl",
  "cut",
  "sort",
  "uniq",
  "cmp",
  "md5sum",
  "sha1sum",
  "sha256sum",
  "ls",
  "rg",
  "grep",
  "egrep",
  "fgrep",
  "sed",
  "awk",
  "file",
  "stat",
];

/// Whether `part` reads the dependency sources as it reads the worktree: a known-safe read by one of the
/// [`SOURCE_READERS`].
pub(super) fn reads_sources(part: &CommandPart) -> bool {
  part.known_safe
    && part.category() == CommandCategory::ReadOnly
    && part.program().is_some_and(|program| SOURCE_READERS.contains(&program))
}

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
    // A repository's own files may hold the credentials of its remote, and its settings name programs.
    let repository = names.iter().any(|name| name.eq_ignore_ascii_case(".git"));
    Some(
      !repository
        && sources.iter().any(|folder| {
          names.len() >= folder.len()
            && folder
              .iter()
              .zip(&names)
              .all(|(pattern, name)| *pattern == "*" || *pattern == name.as_str())
        }),
    )
  })
}
