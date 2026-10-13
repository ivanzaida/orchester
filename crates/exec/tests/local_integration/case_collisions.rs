use std::{fs, process::Command};

use super::{LocalError, LocalGitError, Repo, git};

#[test]
fn local_integration_case_collision_preview_matches_actual_filesystem_lookup() {
  preview_cases("true");
}

#[test]
fn local_integration_case_collision_preview_uses_filesystem_when_git_config_disagrees() {
  preview_cases("false");
}

fn preview_cases(ignore_case: &str) {
  for (incoming, retained) in [
    ("local.txt", "LOCAL.txt"),
    ("cache", "CACHE/ignored.bin"),
    ("cache/incoming.bin", "CACHE"),
    ("nested/local.txt", "NESTED/LOCAL.txt"),
  ] {
    let repo = Repo::new();
    git(repo.root(), &["config", "core.ignoreCase", ignore_case]);
    fs::write(repo.root().join(".gitignore"), b".orchester/\n*.txt\ncache\nCACHE\n").expect("ignores");
    git(repo.root(), &["commit", "-am", "ignore cases"]);
    git(repo.root(), &["checkout", "-b", "task"]);
    fs::write(repo.root().join("selected.txt"), b"task result\n").expect("task");
    git(repo.root(), &["commit", "-am", "task"]);
    git(repo.root(), &["checkout", "main"]);
    if let Some((parent, _)) = incoming.rsplit_once('/') {
      fs::create_dir_all(repo.root().join(parent)).expect("incoming parent");
    }
    fs::write(repo.root().join(incoming), b"incoming tracked\n").expect("incoming");
    git(repo.root(), &["add", "-f", incoming]);
    git(repo.root(), &["commit", "-m", "incoming"]);
    let base = git(repo.root(), &["rev-parse", "HEAD"]);
    git(repo.root(), &["checkout", "task"]);
    if let Some((parent, _)) = retained.rsplit_once('/') {
      fs::create_dir_all(repo.root().join(parent)).expect("retained parent");
    }
    fs::write(repo.root().join(retained), [0, 255, 1]).expect("retained ignored");
    // Observe the actual fixture directory: neither the OS nor core.ignoreCase determines volume behavior.
    let aliases =
      repo.root().join(incoming).exists() || incoming.starts_with("cache/") && repo.root().join("cache").is_file();
    let before = repo.git.snapshot().expect("before");
    let preview = repo.git.preview_sync(&before, &base);
    assert_eq!(repo.git.snapshot().expect("preview unchanged"), before);
    if aliases {
      assert!(
        matches!(preview, Err(LocalGitError::State(LocalError::Unsupported))),
        "{preview:?}"
      );
      assert_eq!(repo.git.snapshot().expect("preflight leaves checkout intact"), before);
    } else {
      let preview = preview.expect("distinct filesystem names allowed");
      let merged = Command::new("git")
        .args(["merge", "--no-ff", "--no-commit", "--no-overwrite-ignore", &base])
        .current_dir(repo.root())
        .output()
        .expect("actual Git merge");
      assert!(merged.status.success(), "distinct filesystem names merge");
      assert_eq!(repo.git.snapshot().expect("merged"), preview);
      assert_eq!(
        fs::read(repo.root().join(incoming)).expect("incoming"),
        b"incoming tracked\n"
      );
    }
    assert_eq!(fs::read(repo.root().join(retained)).expect("retained"), [0, 255, 1]);
  }
}
