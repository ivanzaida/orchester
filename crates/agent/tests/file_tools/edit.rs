//! `edit_file` and `write_file`: read stamps, atomic multi-edits, line endings and BOM, protected paths
//! and atomic writes.

use std::fs;

use orchester_agent::FileTools;
use orchester_core::{
  connection::stream::{FileChange, FileChangeKind},
  native::item::{Item, UserOrigin},
  permission::RepositoryAccess,
};

use crate::{Layout, edit_input, failed, ok, read_input, text, write_input};

fn temporaries(folder: &std::path::Path) -> Vec<String> {
  fs::read_dir(folder)
    .expect("a folder")
    .flatten()
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .filter(|name| name.starts_with(".orchester-write-"))
    .collect()
}

#[test]
fn files_edit_needs_a_current_read() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  let edit = edit_input("src/main.rs", &[("hello", "world", false)]);
  assert!(failed(tools.edit_file(&edit)).contains("has not been read"));
  ok(tools.read_file(&read_input("src/main.rs", None, None)));
  let outcome = tools.edit_file(&edit);
  assert_eq!(
    outcome.changes,
    vec![FileChange {
      path: "src/main.rs".to_owned(),
      kind: FileChangeKind::Modified
    }]
  );
  ok(outcome);
  assert!(layout.read("src/main.rs").contains("world"));
  let again = edit_input("src/main.rs", &[("world", "again", false)]);
  ok(tools.edit_file(&again));
  fs::write(layout.worktree.join("src/main.rs"), "changed by a command\n").expect("file");
  let stale = failed(tools.edit_file(&edit_input("src/main.rs", &[("command", "x", false)])));
  assert!(stale.contains("changed since you read it"), "{stale}");
  assert_eq!(layout.read("src/main.rs"), "changed by a command\n");
}

#[test]
fn files_multi_edit_is_all_or_nothing() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  ok(tools.read_file(&read_input("README.md", None, None)));
  let edit = edit_input("README.md", &[("Readme", "Guide", false), ("missing", "x", false)]);
  let error = failed(tools.edit_file(&edit));
  assert!(error.starts_with("edit 2:"), "{error}");
  assert_eq!(layout.read("README.md"), "# Readme\nhello there\n");
  fs::write(layout.worktree.join("twice.txt"), "a b a\n").expect("file");
  ok(tools.read_file(&read_input("twice.txt", None, None)));
  let ambiguous = failed(tools.edit_file(&edit_input("twice.txt", &[("a", "c", false)])));
  assert!(ambiguous.contains("occurs 2 times"), "{ambiguous}");
  ok(tools.edit_file(&edit_input("twice.txt", &[("a", "c", true)])));
  assert_eq!(layout.read("twice.txt"), "c b c\n");
}

#[test]
fn files_edit_keeps_crlf_and_bom() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  fs::write(layout.worktree.join("win.txt"), "\u{feff}one\r\ntwo\r\nthree\r\n").expect("file");
  ok(tools.read_file(&read_input("win.txt", None, None)));
  ok(tools.edit_file(&edit_input("win.txt", &[("one\ntwo", "uno\ndos", false)])));
  assert_eq!(layout.read("win.txt"), "\u{feff}uno\r\ndos\r\nthree\r\n");
}

#[test]
fn files_write_creates_and_replaces_read_files() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  let created = tools.write_file(&write_input("new/deep/file.txt", "first\n"));
  assert_eq!(created.changes[0].kind, FileChangeKind::Added);
  ok(created);
  assert_eq!(layout.read("new/deep/file.txt"), "first\n");
  ok(tools.write_file(&write_input("new/deep/file.txt", "second\n")));
  ok(tools.edit_file(&edit_input("new/deep/file.txt", &[("second", "third", false)])));
  assert_eq!(layout.read("new/deep/file.txt"), "third\n");
  let unread = failed(tools.write_file(&write_input("README.md", "gone\n")));
  assert!(unread.contains("has not been read"), "{unread}");
  assert!(temporaries(&layout.worktree.join("new/deep")).is_empty());
  let cache = text(&layout.cache.join("notes.txt"));
  ok(tools.write_file(&write_input(&cache, "kept\n")));
}

#[test]
fn files_compaction_drops_read_stamps() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  ok(tools.read_file(&read_input("README.md", None, None)));
  tools.stored(&[Item::user("next", UserOrigin::Resume)]);
  ok(tools.edit_file(&edit_input("README.md", &[("there", "you", false)])));
  tools.stored(&[Item::Compaction {
    summary: "so far".to_owned(),
  }]);
  let error = failed(tools.edit_file(&edit_input("README.md", &[("you", "all", false)])));
  assert!(error.contains("has not been read"), "{error}");
}

#[test]
fn files_protected_and_read_only_paths_are_refused() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  for path in [
    ".git",
    "vendor/.git/config",
    ".orchester/state.txt",
    &text(&layout.index.join("map.md")),
  ] {
    let error = failed(tools.write_file(&write_input(path, "x")));
    assert!(error.contains("protected"), "{path}: {error}");
  }
  let read_only = FileTools::new(layout.scope(RepositoryAccess::ReadOnly), None);
  ok(read_only.read_file(&read_input("README.md", None, None)));
  let error = failed(read_only.edit_file(&edit_input("README.md", &[("Readme", "x", false)])));
  assert!(error.contains("outside the folders this step may write"), "{error}");
  assert_eq!(layout.read("README.md"), "# Readme\nhello there\n");
}

#[test]
fn files_failed_write_leaves_no_partial_file() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  let error = failed(tools.write_file(&write_input("src", "not a folder\n")));
  assert!(error.contains("is a folder"), "{error}");
  assert!(temporaries(&layout.worktree).is_empty());
  assert!(temporaries(&layout.worktree.join("src")).is_empty());
}
