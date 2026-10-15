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
/// While an operation holds the metadata: no folder can be renamed, and outside `objects` none can become a junction.
fn held_during_operation(owned: &Path, outside_objects: &Path) {
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
  // Outside `objects`, which gets no anchor, an empty folder is kept nonempty and cannot become a junction.
  for relative in ["refs", "refs/heads", "logs", "logs/refs", "logs/refs/heads"] {
    assert!(
      local_reparse::set_junction(&owned.join(".git").join(relative), outside_objects).is_err(),
      "in-place child reparse must be denied: {relative}"
    );
  }
  for (path, _) in entries(&owned.join(".git/objects")) {
    assert_ne!(
      path.file_name().and_then(|name| name.to_str()),
      Some(".orchester-local-owner"),
      "nothing of Orchester's in objects"
    );
  }
}

/// Orchester's anchors left under `owned`'s `.git`.
fn anchors(owned: &Path) -> Vec<PathBuf> {
  entries(&owned.join(".git"))
    .into_iter()
    .map(|(path, _)| path)
    .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some(".orchester-local-owner"))
    .collect()
}
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
  let original = session
    .text(&["rev-parse", "HEAD:file.txt"])
    .expect("owned original blob");
  held_during_operation(&owned, &outside_objects);
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
  assert!(anchors(&owned).is_empty(), "the operation removes its anchors");
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
