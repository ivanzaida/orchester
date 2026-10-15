use std::{fs, path::PathBuf};

use orchester_exec::local_git::{LocalGit, digest};

use super::{Repo, git};

const MARKERS: [&str; 7] = [
  "index.lock",
  "rebase-merge",
  "rebase-apply",
  "CHERRY_PICK_HEAD",
  "REVERT_HEAD",
  "BISECT_START",
  "MERGE_HEAD",
];

fn marker_priority(local: &LocalGit) {
  assert_eq!(local.operation().expect("clean checkout"), None);
  let head = git(local.root(), &["rev-parse", "HEAD"]);
  let bytes = format!("{head}\n");
  let paths: Vec<_> = MARKERS
    .iter()
    .map(|marker| {
      PathBuf::from(git(
        local.root(),
        &["rev-parse", "--path-format=absolute", "--git-path", marker],
      ))
    })
    .collect();
  for path in &paths {
    fs::write(path, &bytes).expect("private operation marker");
  }
  for (marker, path) in MARKERS.into_iter().zip(paths) {
    let expected = if marker == "MERGE_HEAD" {
      format!("{marker}:{}", digest(bytes.as_bytes()))
    } else {
      marker.to_owned()
    };
    assert_eq!(local.operation().expect("actual marker"), Some(expected));
    fs::remove_file(path).expect("remove only private fixture marker");
  }
  assert_eq!(local.operation().expect("markers removed"), None);
  assert_eq!(git(local.root(), &["status", "--porcelain"]), "");
}

#[test]
fn local_integration_marker_priority_and_merge_identity_use_actual_main_git_paths() {
  let repo = Repo::new();
  marker_priority(&repo.git);
}

#[test]
fn local_integration_marker_priority_and_merge_identity_use_actual_linked_git_paths_with_spaces() {
  let repo = Repo::new();
  let root = repo.root().join(".orchester/worktrees/task with spaces");
  git(
    repo.root(),
    &["worktree", "add", "-b", "task", root.to_str().expect("private path")],
  );
  let scratch = root.join(".orchester/scratch");
  fs::create_dir_all(&scratch).expect("private scratch");
  let local = LocalGit::new(&root, &scratch).expect("linked Git");
  marker_priority(&local);
}
