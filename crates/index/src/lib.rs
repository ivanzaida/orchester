//! The codebase index of a repository revision: plain-text files an agent reads and searches with
//! its ordinary file tools instead of listing and dumping the repository. It is built in-process
//! from the git objects of the revision (no checkout, no network, no model), with tree-sitter
//! grammars for Rust, TypeScript/JavaScript, Python and Go, and language-agnostic parts for every
//! other file. Only git-tracked files are read; ignored and untracked files are never seen.
//!
//! Each revision has a folder `<repo>/.orchester/index/<revision>/`, outside every worktree:
//!
//! - `tree.txt`: every tracked file, `<path>\t<language>\t<lines>` (`-` for binary files, symbolic links and
//!   submodules);
//! - `symbols.tsv`: one definition per line, `<name>\t<path>\t<line>\t<kind>\t<container>`, sorted by name; the
//!   container is the enclosing item (`impl Worktree`, `class Store`), empty at top level;
//! - `outline/<path>.md` for each file of a parsed language: its items with line ranges, nested (impl → methods, class
//!   → methods, mod → items), each with its signature;
//! - `docs.md`: the headings of tracked Markdown files as `rg -n` prints them, `<path>:<line>:<heading>`;
//! - `map.md`: the packages found in manifests (`Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`) with their
//!   entry files, module tree, test folders and counts, and the top-level folders outside any package;
//! - `manifest.json`: revision, build time, how it was built, languages and counts.
//!
//! Paths are relative to the repository with `/` separators, lines are 1-based. Everything but the
//! build fields of `manifest.json` depends only on the revision's files: an index built from an
//! earlier one (reading only the files `git diff-tree` names) equals one built from scratch.
//! [`CodebaseIndexes`] builds, reuses and removes them.

mod build;
mod git;
mod language;
mod manifest;
mod map;
mod markdown;
mod outline;
mod outline_file;
mod parallel;
mod previous;
mod record;
mod render;
mod store;
mod write;

use std::{io, path::PathBuf};

use thiserror::Error;

pub use self::{
  git::GitError,
  manifest::BuildMode,
  store::{BuildReport, CodebaseIndexes, Ensured, Outcome},
};

#[derive(Debug, Error)]
pub enum IndexError {
  #[error(transparent)]
  Git(#[from] GitError),
  #[error("cannot {action} {path}: {source}")]
  Io {
    action: &'static str,
    path: PathBuf,
    source: io::Error,
  },
  /// The earlier index an incremental build started from does not read back; a full build
  /// replaces it.
  #[error("the index of {revision} cannot be reused: {detail}")]
  Previous { revision: String, detail: String },
}

impl IndexError {
  pub(crate) fn io(action: &'static str, path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> Self {
    let path = path.into();
    move |source| Self::Io { action, path, source }
  }
}
