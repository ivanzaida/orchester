//! Operations change the person's repository only through Git: Orchester itself creates no file in `.git`, while
//! an operation holds it and after, for a selected commit and for the legacy merge.

use std::path::{Path, PathBuf};

use orchester_core::integration::local::LocalPath;
use orchester_exec::programs::Programs;
use orchester_workspace::tasks::integration::merge_in;

use super::{
  snapshots::entries,
  support::{Fixture, git},
};

/// Orchester's names among `.git`'s entries under `root`.
fn orchester_files(root: &Path) -> Vec<PathBuf> {
  entries(&root.join(".git"))
    .into_iter()
    .map(|(path, _)| path)
    .filter(|path| {
      path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(".orchester"))
    })
    .collect()
}

#[test]
fn local_git_a_commit_operation_creates_nothing_in_git_while_held_or_after() {
  let fixture = Fixture::new(false);
  let root = fixture.root();
  std::fs::write(root.join("local.txt"), "base\nselected\n").expect("edit");
  git(root, &["add", "local.txt"]);
  let before = entries(&root.join(".git"));
  let path = LocalPath::new("local.txt").expect("path");
  let mut session = fixture.open();
  assert_eq!(
    entries(&root.join(".git")),
    before,
    "the operation holds .git without adding to it"
  );
  let tree = session.selected_commit_tree(std::slice::from_ref(&path)).expect("tree");
  session.commit(&[path], &tree, "selected commit").expect("commit");
  assert!(orchester_files(root).is_empty(), "while held after the commit");
  drop(session);
  assert!(orchester_files(root).is_empty(), "after the operation");
  assert_eq!(git(root, &["log", "-1", "--format=%s"]), "selected commit");
}

#[test]
fn local_git_the_legacy_merge_creates_nothing_in_git() {
  let fixture = Fixture::new(false);
  let root = fixture.root();
  let source = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  merge_in(root, &Programs::processes(), (fixture.run, None)).expect("legacy merge");
  assert_eq!(git(root, &["rev-parse", "HEAD"]), source);
  assert!(orchester_files(root).is_empty());
}
