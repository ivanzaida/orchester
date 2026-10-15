//! Commit hooks run before the branch moves: a hook that rewrites the staged content or the reviewed message stops
//! the commit with what it changed, and the branch, the checkout's index and its files stay as reviewed. A
//! post-commit hook that moves HEAD after the reviewed commit landed is recorded for the person.
use std::{fs, path::Path};

use orchester_core::integration::local::{HookChange, LocalError, LocalPath, LocalPhase};
use orchester_workspace::tasks::local_integration::IntegrationError;

use super::support::{Fixture, git};

fn hook(root: &Path, name: &str, script: &str) {
  let path = root.join(".git/hooks").join(name);
  fs::write(&path, format!("#!/bin/sh\n{script}\n")).expect("hook");
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable hook");
  }
}

/// A fixture with `local.txt` staged for the commit, and what the checkout looked like before it.
fn staged() -> (Fixture, String, String) {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), b"selected staged\n").expect("selected");
  git(fixture.root(), &["add", "local.txt"]);
  let head = git(fixture.root(), &["rev-parse", "HEAD"]);
  let index = git(fixture.root(), &["ls-files", "--stage"]);
  (fixture, head, index)
}

fn commit(fixture: &Fixture) -> (Result<(), IntegrationError>, Option<HookChange>, LocalPhase, String) {
  let path = LocalPath::new("local.txt").expect("path");
  let mut session = fixture.open();
  let tree = session.selected_commit_tree(std::slice::from_ref(&path)).expect("tree");
  let result = session.commit(&[path], &tree, "selected commit");
  let operation = session.operation();
  (result, operation.hook_change().cloned(), operation.phase(), tree)
}

#[test]
fn local_hooks_rewriting_the_index_stop_the_commit_before_the_branch_moves() {
  let (fixture, head, index) = staged();
  hook(
    fixture.root(),
    "pre-commit",
    "echo hooked >> local.txt && git add local.txt",
  );
  let (result, change, phase, _) = commit(&fixture);
  assert!(
    matches!(result, Err(IntegrationError::State(LocalError::HookChanged))),
    "{result:?}"
  );
  assert_eq!(
    change,
    Some(HookChange::Stopped {
      paths: vec![LocalPath::new("local.txt").expect("path")],
      message: false,
    })
  );
  assert_eq!(phase, LocalPhase::Review);
  assert_eq!(
    git(fixture.root(), &["rev-parse", "HEAD"]),
    head,
    "the branch did not move"
  );
  assert_eq!(
    git(fixture.root(), &["ls-files", "--stage"]),
    index,
    "the index is as reviewed"
  );
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("file"),
    b"selected staged\n",
    "the hook wrote only into its copy"
  );
}

#[test]
fn local_hooks_rewriting_the_message_stop_the_commit_before_the_branch_moves() {
  let (fixture, head, index) = staged();
  hook(fixture.root(), "commit-msg", "echo 'Change-Id: hooked' >> \"$1\"");
  let (result, change, ..) = commit(&fixture);
  assert!(
    matches!(result, Err(IntegrationError::State(LocalError::HookChanged))),
    "{result:?}"
  );
  assert_eq!(
    change,
    Some(HookChange::Stopped {
      paths: Vec::new(),
      message: true,
    })
  );
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), head);
  assert_eq!(git(fixture.root(), &["ls-files", "--stage"]), index);
}

#[test]
fn local_hooks_that_keep_the_commit_let_the_branch_move_to_the_reviewed_tree() {
  let (fixture, head, _) = staged();
  hook(fixture.root(), "pre-commit", "exit 0");
  hook(fixture.root(), "commit-msg", "grep -q 'selected commit' \"$1\"");
  let (result, change, _, tree) = commit(&fixture);
  result.expect("commit");
  assert_eq!(change, None);
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD^"]), head);
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD^{tree}"]), tree);
  assert_eq!(git(fixture.root(), &["log", "-1", "--format=%B"]), "selected commit");
  assert_eq!(git(fixture.root(), &["status", "--porcelain", "--", "local.txt"]), "");
}

#[test]
fn local_hooks_that_fail_stop_the_commit_with_nothing_moved() {
  let (fixture, head, index) = staged();
  hook(fixture.root(), "pre-commit", "exit 1");
  let (result, change, phase, _) = commit(&fixture);
  assert!(result.is_err());
  assert_eq!(change, None);
  assert_eq!(phase, LocalPhase::Review);
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), head);
  assert_eq!(git(fixture.root(), &["ls-files", "--stage"]), index);
}

#[test]
fn local_hooks_that_move_head_after_the_commit_are_recorded_for_the_person() {
  let (fixture, head, _) = staged();
  // `--no-verify` does not skip post-commit: the hook's own amend would run it again, so it amends once.
  hook(
    fixture.root(),
    "post-commit",
    "[ -n \"$AMENDED\" ] || AMENDED=1 git commit --amend --no-verify --allow-empty -m 'amended by hook' >/dev/null",
  );
  let (result, change, ..) = commit(&fixture);
  result.expect("the reviewed commit landed");
  let moved = git(fixture.root(), &["rev-parse", "HEAD"]);
  let Some(HookChange::MovedHead { commit, head: recorded }) = change else {
    panic!("the moved HEAD is recorded: {change:?}");
  };
  assert_eq!(recorded, moved, "the record names where the hook moved HEAD");
  assert_ne!(commit, moved, "and the reviewed commit it moved from");
  assert_eq!(git(fixture.root(), &["rev-parse", &format!("{commit}^")]), head);
  assert_eq!(git(fixture.root(), &["log", "-1", "--format=%s"]), "amended by hook");
}
