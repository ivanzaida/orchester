//! Where other package managers keep the code they install (ORC-95), read like the toolchains' dependency sources
//! ([`super::toolchains`]): a `node_modules` folder (npm, pnpm, yarn; a project's or the global one), Python's
//! `site-packages` and `dist-packages`, and Go's module cache (`<home>/go/pkg/mod`). It is code published for everyone,
//! also where it lies among the home folder's credential locations (`AppData/Roaming/npm/node_modules`,
//! `.local/lib/python3.12/site-packages`, `.nvm/versions/node/*/lib/node_modules`). A `.git` folder in it, and a file
//! that may keep a registry's token (`.npmrc`, `.yarnrc`, `.pypirc`, `.netrc`, `.env…`), are not. Paths are matched
//! resolved, so a link in it that leads elsewhere is placed where it leads.

use std::path::{Path, PathBuf};

/// The folders a package manager installs packages into, by name.
const PACKAGE_FOLDERS: [&str; 3] = ["node_modules", "site-packages", "dist-packages"];
/// Go's module cache in a home folder.
const GO_MODULES: [&str; 3] = ["go", "pkg", "mod"];
/// Names in a package folder that may hold a token or name a program.
const KEPT_OUT: [&str; 5] = [".git", ".npmrc", ".yarnrc", ".pypirc", ".netrc"];

/// Whether `path` (resolved) lies in a package folder, or in Go's module cache of one of `homes`, and in nothing
/// [`KEPT_OUT`] there.
pub(super) fn in_package_sources(path: &Path, homes: &[PathBuf]) -> bool {
  let names: Vec<String> = path
    .components()
    .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
    .collect();
  let package = names
    .iter()
    .position(|name| PACKAGE_FOLDERS.contains(&name.as_str()))
    .or_else(|| {
      homes.iter().find_map(|home| {
        let depth = home.components().count();
        let rest = names.get(depth..)?;
        (path.starts_with(home) && rest.len() >= GO_MODULES.len() && rest[..GO_MODULES.len()] == GO_MODULES)
          .then_some(depth)
      })
    });
  package.is_some_and(|at| {
    names[at..]
      .iter()
      .all(|name| !KEPT_OUT.contains(&name.as_str()) && !name.starts_with(".env"))
  })
}
