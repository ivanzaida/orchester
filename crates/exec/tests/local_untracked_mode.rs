#![cfg(unix)]
use std::{
  fs,
  os::unix::fs::PermissionsExt,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_core::integration::local::LocalPath;
use orchester_exec::local_git::copy_untracked_file;
fn git(root: &Path, args: &[&str]) -> String {
  let output = Command::new("git")
    .args(args)
    .current_dir(root)
    .output()
    .expect("private Git");
  assert!(output.status.success());
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}
#[test]
fn local_integration_untracked_executable_copy_commits_100755_and_preserves_original_permissions() {
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
  fs::create_dir_all(&scratch).expect("scratch");
  let source = tempfile::tempdir_in(&scratch).expect("private source");
  let copy = tempfile::tempdir_in(&scratch).expect("private commit copy");
  let script = source.path().join("run.sh");
  let bytes = b"#!/bin/sh\nprintf 'private fixture'\n";
  fs::write(&script, bytes).expect("untracked executable");
  fs::set_permissions(&script, fs::Permissions::from_mode(0o750)).expect("working permissions");
  let path = LocalPath::new("run.sh").expect("path");
  copy_untracked_file(source.path(), copy.path(), &path, true).expect("production private-copy transfer");
  git(copy.path(), &["init", "-b", "main"]);
  git(copy.path(), &["config", "user.name", "Private mode regression"]);
  git(copy.path(), &["config", "user.email", "mode@example.invalid"]);
  git(copy.path(), &["add", "--", "run.sh"]);
  git(copy.path(), &["commit", "-m", "selected executable"]);
  assert!(git(copy.path(), &["ls-tree", "HEAD", "--", "run.sh"]).starts_with("100755 "));
  assert_eq!(fs::read(&script).expect("original bytes"), bytes);
  assert_eq!(
    fs::metadata(&script).expect("original mode").permissions().mode() & 0o777,
    0o750
  );
}
