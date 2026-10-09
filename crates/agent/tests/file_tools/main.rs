//! The file and search tool executors on real file systems (`docs/architecture/native-agent.md` §8,
//! layer 3): temporary worktrees, links that lead out, protected folders, line endings, stale read
//! stamps and atomic writes; and Orchester's line edits and term searches for agent CLIs.
//!
//! `cargo test -p orchester-agent --test file_tools files_`

mod edit;
mod line_edits;
mod links;
mod listing;
mod read;
mod search;
mod terms;

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_agent::FileTools;
use orchester_core::{
  native::{
    scope::{FileScope, ScopeRoots},
    session::ToolOutcome,
    tools::{EditFile, ListFiles, ReadFile, Search, ToolInput, WriteFile, parse_call},
  },
  permission::{FolderBounds, RepositoryAccess, WritableFolder},
};
use serde_json::{Value, json};
use tempfile::TempDir;

/// `repo` with a run worktree, the code index, a skills folder and a writable folder under
/// `.orchester/`, and a folder `outside`. The worktree holds `src/main.rs`, `README.md`, a `.git` file,
/// a hidden `.env` and a `.gitignore` that ignores `target/`.
pub struct Layout {
  _root: TempDir,
  pub root: PathBuf,
  pub worktree: PathBuf,
  pub index: PathBuf,
  pub cache: PathBuf,
  pub outside: PathBuf,
}

impl Layout {
  pub fn temporary() -> Self {
    let dir = tempfile::tempdir().expect("temporary folder");
    let canonical = fs::canonicalize(dir.path()).expect("canonical root");
    let text = canonical.to_str().expect("UTF-8 path");
    let root = PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(text));
    let orchester = root.join("repo/.orchester");
    let worktree = orchester.join("worktrees/run");
    for folder in [
      worktree.join("src/nested"),
      worktree.join("target"),
      orchester.join("index/rev"),
      orchester.join("skills/exec"),
      orchester.join("cache"),
      root.join("outside"),
    ] {
      fs::create_dir_all(folder).expect("folder");
    }
    let files = [
      (
        worktree.join("src/main.rs"),
        "fn main() {\n    println!(\"hello\");\n}\n",
      ),
      (worktree.join("src/nested/lib.rs"), "pub fn hello() {}\n"),
      (worktree.join("README.md"), "# Readme\nhello there\n"),
      (worktree.join(".git"), "gitdir: elsewhere\n"),
      (worktree.join(".env"), "hello=hidden\n"),
      (worktree.join(".gitignore"), "target/\n"),
      (worktree.join("target/out.txt"), "hello build\n"),
      (
        orchester.join("index/rev/symbols.tsv"),
        "hello\tsrc/nested/lib.rs\t1\tfunction\t\n",
      ),
      (root.join("outside/secret.txt"), "secret\n"),
    ];
    for (path, content) in files {
      fs::write(path, content).expect("file");
    }
    Self {
      worktree,
      index: orchester.join("index/rev"),
      cache: orchester.join("cache"),
      outside: root.join("outside"),
      root,
      _root: dir,
    }
  }

  /// The scope of a step with `access`, granted the writable folder `.orchester/cache`.
  pub fn scope(&self, access: RepositoryAccess) -> FileScope {
    let bounds = FolderBounds::new(&self.root.join("repo"), None).expect("bounds");
    let folder = WritableFolder::new(".orchester/cache", &bounds).expect("writable folder");
    let resolved = [folder.resolve(&bounds).expect("resolved folder")];
    FileScope::new(&ScopeRoots {
      worktree: &self.worktree,
      access,
      code_index: Some(&self.index),
      skills: None,
      writable_folders: &resolved,
    })
    .expect("scope")
  }

  /// File tools that edit the worktree, without ripgrep.
  pub fn tools(&self) -> FileTools {
    FileTools::new(self.scope(RepositoryAccess::Write), None)
  }

  pub fn read(&self, relative: &str) -> String {
    fs::read_to_string(self.worktree.join(relative)).expect("a file")
  }
}

/// ripgrep as Orchester finds it; the tests that need it fail without it.
pub fn ripgrep() -> PathBuf {
  orchester_exec::ripgrep::locate(
    std::env::var_os("PATH").as_deref(),
    std::env::var_os("LOCALAPPDATA").as_deref(),
    &[],
  )
  .expect("these tests need ripgrep (`rg`) on PATH")
  .path
}

fn parsed(name: &str, input: &Value) -> ToolInput {
  parse_call(name, input).unwrap_or_else(|error| panic!("{name}: {error}"))
}

pub fn read_input(path: &str, offset: Option<u32>, limit: Option<u32>) -> ReadFile {
  match parsed("read_file", &json!({"path": path, "offset": offset, "limit": limit})) {
    ToolInput::ReadFile(input) => input,
    other => panic!("{other:?}"),
  }
}

pub fn edit_input(path: &str, edits: &[(&str, &str, bool)]) -> EditFile {
  let edits: Vec<Value> = edits
    .iter()
    .map(|(old, new, all)| json!({"old": old, "new": new, "replace_all": all}))
    .collect();
  match parsed("edit_file", &json!({"path": path, "edits": edits})) {
    ToolInput::EditFile(input) => input,
    other => panic!("{other:?}"),
  }
}

pub fn write_input(path: &str, content: &str) -> WriteFile {
  match parsed("write_file", &json!({"path": path, "content": content})) {
    ToolInput::WriteFile(input) => input,
    other => panic!("{other:?}"),
  }
}

pub fn list_input(pattern: Option<&str>, path: Option<&str>) -> ListFiles {
  match parsed("list_files", &json!({"pattern": pattern, "path": path})) {
    ToolInput::ListFiles(input) => input,
    other => panic!("{other:?}"),
  }
}

pub fn search_input(pattern: &str, path: Option<&str>, glob: Option<&str>, context: Option<u32>) -> Search {
  let input = json!({"pattern": pattern, "path": path, "glob": glob, "case_insensitive": null, "context": context});
  match parsed("search", &input) {
    ToolInput::Search(input) => input,
    other => panic!("{other:?}"),
  }
}

/// The content of a successful outcome.
pub fn ok(outcome: ToolOutcome) -> String {
  assert!(!outcome.is_error, "{}", outcome.content);
  outcome.content
}

/// The content of a failed outcome.
pub fn failed(outcome: ToolOutcome) -> String {
  assert!(outcome.is_error, "expected an error, got: {}", outcome.content);
  outcome.content
}

/// `path` as a tool call carries it.
pub fn text(path: &Path) -> String {
  path.to_str().expect("UTF-8 path").to_owned()
}
