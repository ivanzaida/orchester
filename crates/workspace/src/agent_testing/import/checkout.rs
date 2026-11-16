//! Reading the project's checkout for the import, and nothing else: its folders are listed and its files read, no
//! program is started (neither git, cargo nor a package manager) and no link or junction is followed while listing.
//! Every file is opened through the scope agents' file tools use (`FileScope::repository_readable`), so a file that
//! leads out of the checkout, `.git` and `.orchester` are refused.

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  native::scope::{FileScope, ScopeRoots},
  permission::RepositoryAccess,
};

use super::Unreadable;
use crate::messages::Message;

/// How many folders below the checkout the listing goes.
const MAX_DEPTH: usize = 6;
/// The most entries listed; past them the listing stops and says so.
const MAX_ENTRIES: usize = 50_000;
/// The largest file read; a manifest or compose file is far smaller.
pub(super) const MAX_FILE_BYTES: u64 = 512 * 1024;
/// Folders that hold what was installed, built or vendored, or fixtures of tests, rather than the project's own apps.
const PASSED_OVER: &[&str] = &[
  "node_modules",
  "bower_components",
  "target",
  "vendor",
  "dist",
  "build",
  "out",
  "tests",
  "test",
  "__tests__",
  "fixtures",
  "testdata",
];
/// The `docker compose` file names Compose itself looks for.
pub(super) const COMPOSE_FILES: &[&str] = &[
  "compose.yaml",
  "compose.yml",
  "docker-compose.yaml",
  "docker-compose.yml",
];
pub(super) const PACKAGE_FILE: &str = "package.json";
pub(super) const CARGO_FILE: &str = "Cargo.toml";

/// The checkout as the import reads it.
pub(super) struct Checkout {
  scope: FileScope,
}

/// The files the import reads, by their paths relative to the checkout with `/` between their parts, in order.
#[derive(Default)]
pub(super) struct Listing {
  pub files: Vec<String>,
  pub unreadable: Vec<Unreadable>,
  /// The listing stopped at [`MAX_ENTRIES`].
  pub truncated: bool,
}

impl Checkout {
  pub(super) fn open(root: &Path) -> Result<Self, Message> {
    let scope = FileScope::new(&ScopeRoots {
      worktree: root,
      access: RepositoryAccess::ReadOnly,
      code_index: None,
      skills: None,
      writable_folders: &[],
      run_storage: None,
    })
    .map_err(|error| Message::new("services.import.errors.checkout").with("reason", error.to_string()))?;
    Ok(Self { scope })
  }

  /// The compose files, `package.json` and `Cargo.toml` files of the checkout.
  pub(super) fn list(&self) -> Listing {
    let mut listing = Listing::default();
    let mut seen = 0;
    self.walk(self.scope.worktree(), "", 0, (&mut listing, &mut seen));
    listing
  }

  fn walk(&self, folder: &Path, relative: &str, depth: usize, (listing, seen): (&mut Listing, &mut usize)) {
    let entries = match fs::read_dir(folder) {
      Ok(entries) => entries,
      Err(error) => return listing.unreadable.push(unreadable(relative, &error)),
    };
    let mut found = Vec::new();
    for entry in entries {
      match entry.and_then(|entry| Ok((entry.file_name(), entry.file_type()?))) {
        Ok(found_entry) => found.push(found_entry),
        Err(error) => listing.unreadable.push(unreadable(relative, &error)),
      }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, kind) in found {
      *seen += 1;
      if *seen > MAX_ENTRIES {
        listing.truncated = true;
        return;
      }
      // A name that is not Unicode is none of the manifests' names, nor a folder a manifest could name.
      let Some(name) = name.to_str() else { continue };
      let path = if relative.is_empty() {
        name.to_owned()
      } else {
        format!("{relative}/{name}")
      };
      // A link or junction is passed over, so the listing never leaves the checkout or goes round in a loop.
      if kind.is_symlink() {
        continue;
      }
      if kind.is_dir() && depth < MAX_DEPTH && !name.starts_with('.') && !PASSED_OVER.contains(&name) {
        self.walk(&folder.join(name), &path, depth + 1, (listing, seen));
      } else if kind.is_file() && manifest(name) {
        listing.files.push(path);
      }
    }
  }

  /// The text of the checkout's file `path`.
  pub(super) fn read(&self, path: &str) -> Result<String, Message> {
    let resolved = self.resolve(path)?;
    let failed =
      |error: std::io::Error| Message::new("services.import.unreadable.io").with("reason", error.to_string());
    let size = fs::metadata(&resolved).map_err(failed)?.len();
    if size > MAX_FILE_BYTES {
      return Err(
        Message::new("services.import.unreadable.tooLarge")
          .with("size", size.to_string())
          .with("max", MAX_FILE_BYTES.to_string()),
      );
    }
    let bytes = fs::read(&resolved).map_err(failed)?;
    String::from_utf8(bytes).map_err(|_| Message::new("services.import.unreadable.notText"))
  }

  /// Whether the checkout has a file at `path`.
  pub(super) fn is_file(&self, path: &str) -> bool {
    self
      .resolve(path)
      .is_ok_and(|resolved| fs::metadata(resolved).is_ok_and(|metadata| metadata.is_file()))
  }

  /// The names of the files right in the checkout's folder `path`, in order. None when it is no folder or cannot be
  /// listed: Cargo, finding no binaries there, would find none either.
  pub(super) fn files_in(&self, path: &str) -> Vec<String> {
    self.entries_in(path, |kind| kind.is_file())
  }

  /// The names of the folders right in the checkout's folder `path`, in order, links passed over.
  pub(super) fn folders_in(&self, path: &str) -> Vec<String> {
    self.entries_in(path, |kind| kind.is_dir())
  }

  fn entries_in(&self, path: &str, wanted: impl Fn(&fs::FileType) -> bool) -> Vec<String> {
    let Ok(resolved) = self.resolve(path) else {
      return Vec::new();
    };
    let Ok(entries) = fs::read_dir(resolved) else {
      return Vec::new();
    };
    let mut names: Vec<String> = entries
      .flatten()
      .filter(|entry| entry.file_type().is_ok_and(|kind| !kind.is_symlink() && wanted(&kind)))
      .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
      .collect();
    names.sort();
    names
  }

  fn resolve(&self, path: &str) -> Result<PathBuf, Message> {
    self
      .scope
      .repository_readable(path)
      .map_err(|error| Message::new("services.import.unreadable.outside").with("reason", error.to_string()))
  }
}

fn manifest(name: &str) -> bool {
  COMPOSE_FILES.contains(&name) || name == PACKAGE_FILE || name == CARGO_FILE
}

fn unreadable(folder: &str, error: &std::io::Error) -> Unreadable {
  Unreadable {
    file: if folder.is_empty() {
      ".".to_owned()
    } else {
      folder.to_owned()
    },
    reason: Message::new("services.import.unreadable.io").with("reason", error.to_string()),
  }
}
