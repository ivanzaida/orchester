//! Deterministic private directory-substitution regression; never targets a live repository.
#![cfg(windows)]
use std::{
  fs,
  io::Write,
  path::{Path, PathBuf},
  process::{Command, Stdio},
};

use orchester_exec::local_git::LocalGit;
#[path = "support/local_reparse.rs"]
mod local_reparse;

fn git(root: &Path, args: &[&str], input: Option<&[u8]>) -> String {
  let mut child = Command::new("git")
    .args(args)
    .current_dir(root)
    .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("private Git");
  if let Some(input) = input {
    child.stdin.take().expect("stdin").write_all(input).expect("input");
  }
  let output = child.wait_with_output().expect("Git outcome");
  assert!(
    output.status.success(),
    "private Git failed: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout)
    .expect("fixture ASCII")
    .trim()
    .to_owned()
}
fn init(root: &Path) {
  fs::create_dir(root).expect("private repo");
  git(root, &["init", "-b", "main"], None);
  git(root, &["config", "user.name", "Private fixture"], None);
  git(root, &["config", "user.email", "private@example.invalid"], None);
  fs::write(root.join("file.txt"), b"original owned content").expect("file");
  git(root, &["add", "file.txt"], None);
  git(root, &["commit", "-m", "private initial"], None);
}
fn entries(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
  let mut result = Vec::new();
  let mut pending = vec![root.to_owned()];
  while let Some(directory) = pending.pop() {
    for entry in fs::read_dir(directory).expect("private entries") {
      let entry = entry.expect("entry");
      let relative = entry.path().strip_prefix(root).expect("relative").to_owned();
      if entry.file_type().expect("type").is_dir() {
        result.push((relative, Vec::new()));
        pending.push(entry.path());
      } else {
        result.push((relative, fs::read(entry.path()).expect("private bytes")));
      }
    }
  }
  result.sort();
  result
}
/// While an operation holds the metadata no folder of `.git` can be renamed.
fn held_during_operation(owned: &Path) {
  for relative in [
    "objects",
    "refs",
    "refs/heads",
    "logs",
    "logs/refs",
    "logs/refs/heads",
    "objects/pack",
    "objects/info",
  ] {
    let child = owned.join(".git").join(relative);
    assert!(
      fs::rename(&child, owned.join("scratch").join(relative.replace('/', "-"))).is_err(),
      "child rename must be denied: {relative}"
    );
  }
}

/// The marker earlier builds left in the folders they held.
const LEFTOVER: &str = ".orchester-local-owner";

#[test]
fn local_integration_git_child_chains_remain_owned_before_later_real_git_io() {
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
  fs::create_dir_all(&scratch).expect("scratch");
  let dir = tempfile::tempdir_in(&scratch).expect("private fixture");
  let outside = dir.path().join("outside-control");
  let unguarded = dir.path().join("unguarded");
  let owned = dir.path().join("owned");
  init(&outside);
  init(&unguarded);
  init(&owned);
  let outside_objects = outside.join(".git/objects");
  let control_objects = unguarded.join(".git/objects");
  fs::rename(&control_objects, unguarded.join(".git/objects-original")).expect("unguarded swap");
  fs::create_dir(&control_objects).expect("private junction entry");
  local_reparse::set_junction(&control_objects, &outside_objects).expect("actual unguarded private child junction");
  let control = git(
    &unguarded,
    &["hash-object", "-w", "--stdin"],
    Some(b"positive control outside write"),
  );
  assert!(outside_objects.join(&control[..2]).join(&control[2..]).is_file());
  assert_eq!(
    git(&outside, &["cat-file", "blob", &control], None),
    "positive control outside write"
  );
  fs::remove_dir(&control_objects).expect("remove only private junction entry");
  let before = entries(&outside.join(".git"));
  let owned_before = entries(&owned.join(".git"));
  fs::create_dir(owned.join("scratch")).expect("owned scratch");
  let mut session = LocalGit::new(&owned, &owned.join("scratch")).expect("verified owned preflight");
  session.snapshot().expect("a read");
  assert_eq!(
    entries(&owned.join(".git")),
    owned_before,
    "opening and reading write nothing into .git"
  );
  session.own_metadata().expect("an operation holds the metadata");
  assert_eq!(
    entries(&owned.join(".git")),
    owned_before,
    "holding the metadata creates nothing in .git"
  );
  let original = session
    .text(&["rev-parse", "HEAD:file.txt"])
    .expect("owned original blob");
  held_during_operation(&owned);
  let object = session
    .put_blob(b"later owned object write")
    .expect("real later Git write");
  assert_eq!(
    session.blob(&object).expect("real later Git read"),
    b"later owned object write"
  );
  assert_eq!(
    session.blob(&original).expect("original actual blob"),
    b"original owned content"
  );
  session
    .text(&["fsck", "--full", "--no-progress"])
    .expect("owned metadata preserves normal Git integrity checks");
  assert_eq!(
    entries(&outside.join(".git")),
    before,
    "outside bytes and object entries unchanged after guarded operations"
  );
  drop(session);
  assert!(
    !entries(&owned.join(".git"))
      .iter()
      .any(|(path, _)| path.file_name().and_then(|name| name.to_str()) == Some(LEFTOVER)),
    "nothing of Orchester's in .git after the operation"
  );
}

#[test]
fn local_integration_operation_removes_markers_earlier_builds_left_in_git() {
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
  fs::create_dir_all(&scratch).expect("scratch");
  let dir = tempfile::tempdir_in(&scratch).expect("private fixture");
  let owned = dir.path().join("owned");
  init(&owned);
  let task = owned.join(".orchester/worktrees/task");
  fs::create_dir_all(task.parent().expect("parent")).expect("task parent");
  git(
    &owned,
    &["worktree", "add", "-b", "task", task.to_str().expect("path")],
    None,
  );
  let bucket = fs::read_dir(owned.join(".git/objects"))
    .expect("objects")
    .map(|entry| entry.expect("entry").path())
    .find(|path| {
      path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.len() == 2)
    })
    .expect("a loose-object bucket");
  let left = [
    owned.join(".git"),
    bucket,
    owned.join(".git/refs/heads"),
    owned.join(".git/logs"),
    owned.join(".git/worktrees/task"),
  ]
  .map(|folder| folder.join(LEFTOVER));
  for marker in &left {
    fs::write(marker, b"").expect("an earlier build's marker");
  }
  // Another name is never Orchester's to remove.
  fs::write(owned.join(".git/refs/heads/.orchester-local-owner-not"), b"").expect("a person's file");
  fs::create_dir(owned.join("scratch")).expect("owned scratch");
  let mut session = LocalGit::new(&owned, &owned.join("scratch")).expect("verified owned preflight");
  session.own_metadata().expect("an operation holds the metadata");
  for marker in &left {
    assert!(!marker.exists(), "{} is removed", marker.display());
  }
  assert!(owned.join(".git/refs/heads/.orchester-local-owner-not").exists());
}
#[test]
fn local_integration_canonical_linked_metadata_and_nested_branch_parents_are_held() {
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
  fs::create_dir_all(&scratch).expect("scratch");
  let dir = tempfile::tempdir_in(scratch).expect("private fixture");
  let root = dir.path().join("owned");
  init(&root);
  let task = root.join(".orchester/worktrees/task");
  fs::create_dir_all(task.parent().expect("parent")).expect("private task parent");
  git(
    &root,
    &["worktree", "add", "-b", "orc/task/result", task.to_str().expect("path")],
    None,
  );
  fs::create_dir(task.join("scratch")).expect("scratch");
  let canonical = task.canonicalize().expect("canonical Windows path");
  let mut session = LocalGit::new(&canonical, &canonical.join("scratch")).expect("owned linked project metadata");
  session.own_metadata().expect("an operation holds the metadata");
  for relative in [
    "objects",
    "refs/heads/orc",
    "refs/heads/orc/task",
    "logs/refs/heads/orc",
    "logs/refs/heads/orc/task",
  ] {
    let original = root.join(".git").join(relative);
    assert!(fs::rename(&original, root.join(relative.replace('/', "-"))).is_err());
  }
  let object = session
    .put_blob(b"linked task object")
    .expect("write through actual common directory");
  assert_eq!(session.blob(&object).expect("actual read"), b"linked task object");
  assert_eq!(
    session.text(&["symbolic-ref", "--short", "HEAD"]).expect("branch"),
    "orc/task/result"
  );
}
