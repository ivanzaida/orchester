//! The calls both fake CLIs make of Orchester's file tools.
//!
//! - "Use Orchester's tools" ([`external`]): outside the worktree, in the repository's `deps-a` and `deps-b` folders
//!   next to its `.orchester` folder: a search and a read in `deps-a`, a read in `deps-b`, and a read of the
//!   repository's `.git/config`, which is refused before anybody is asked.
//! - "Use the file tools" ([`writing`], [`reading`]): in the worktree, each refusal included: an edit before the file
//!   is read, then after; an edit whose expected lines are stale; a new file and one that exists; a search; reads and
//!   creates that leave the worktree or reach protected folders; a read of a file larger than an answer holds.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// A folder of the repository the worktree belongs to, outside the worktree.
pub fn repository_folder(worktree: &Path, name: &str) -> PathBuf {
  worktree
    .ancestors()
    .nth(3)
    .map_or_else(PathBuf::new, |repository| repository.join(name))
}

fn text(path: &Path) -> String {
  path.to_string_lossy().into_owned()
}

/// Each call of "Use Orchester's tools": its id, its tool and its input.
pub fn external(worktree: &Path) -> Vec<(&'static str, &'static str, Value)> {
  let (first, second) = (
    repository_folder(worktree, "deps-a"),
    repository_folder(worktree, "deps-b"),
  );
  let git = repository_folder(worktree, ".git").join("config");
  vec![
    (
      "toolu_search_out",
      "search_external_file",
      json!({"base_directory": text(&first), "term": "outside line"}),
    ),
    (
      "toolu_read_out",
      "read_external_file",
      json!({"filename": text(&first.join("notes.txt"))}),
    ),
    (
      "toolu_read_other",
      "read_external_file",
      json!({"filename": text(&second.join("notes.txt")), "start_line": 2, "length": 1}),
    ),
    ("toolu_read_git", "read_external_file", json!({"filename": text(&git)})),
  ]
}

fn edit(start: u32, expected: &str, content: &str) -> Value {
  json!({"filename": "README.md", "start_line": start, "end_line": start, "expected": expected, "content": content})
}

/// Each call of "Use the file tools", in a writing step or in a `read_only` one.
pub fn files(read_only: bool) -> Vec<(&'static str, &'static str, Value)> {
  if read_only {
    return vec![
      ("toolu_read", "read_file", json!({"filename": "README.md"})),
      ("toolu_edit", "edit_file", edit(1, "hi", "ho")),
      (
        "toolu_create",
        "create_file",
        json!({"filename": "new.md", "content": "x"}),
      ),
    ];
  }
  vec![
    ("toolu_list", "list_files", json!({"pattern": null, "path": null})),
    ("toolu_edit_unread", "edit_file", edit(1, "hi", "hi there")),
    ("toolu_read", "read_file", json!({"filename": "README.md"})),
    ("toolu_edit", "edit_file", edit(1, "hi", "hi there")),
    ("toolu_edit_stale", "edit_file", edit(1, "hi", "ho")),
    (
      "toolu_create",
      "create_file",
      json!({"filename": "notes/new.md", "content": "new notes\n"}),
    ),
    (
      "toolu_create_existing",
      "create_file",
      json!({"filename": "README.md", "content": "x"}),
    ),
    (
      "toolu_search",
      "search_file",
      json!({"term": "hi there", "limit_files": null}),
    ),
    ("toolu_up", "read_file", json!({"filename": "../outside.txt"})),
    (
      "toolu_git",
      "create_file",
      json!({"filename": ".git/config", "content": "x"}),
    ),
    (
      "toolu_state",
      "create_file",
      json!({"filename": ".orchester/x.txt", "content": "x"}),
    ),
    ("toolu_wide", "read_file", json!({"filename": "wide.txt"})),
  ]
}
