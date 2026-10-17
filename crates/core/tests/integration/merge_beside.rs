//! Merging a run into a checkout that holds uncommitted work: what the merge does not reach stays as
//! it is, what it would overwrite refuses it and is named, and the temporary worktree of a merge
//! commit never stays behind.

use std::fs;

use orchester_core::{
  integration::{MergeError, MergeKind, merge_run},
  run::Run,
};

use crate::{
  merge::repo_with_run_work,
  support::{Ending, Processes, RUN_BRANCH, Repo, git_program, now, run, task},
};

/// Uncommitted work the run does not touch: an edited `README.md`, a staged new file and an
/// untracked one.
fn leave_unrelated_work(repo: &Repo) {
  fs::write(repo.root.join("README.md"), "edited, not committed\n").expect("edit");
  fs::write(repo.root.join("staged.txt"), "staged\n").expect("staged file");
  repo.git(&["add", "staged.txt"]);
  fs::write(repo.root.join("notes.txt"), "untracked\n").expect("untracked file");
}

/// The unrelated work [`leave_unrelated_work`] left is there as it was.
fn assert_unrelated_work_kept(repo: &Repo) {
  assert_eq!(
    fs::read_to_string(repo.root.join("README.md")).expect("still edited"),
    "edited, not committed\n"
  );
  assert_eq!(
    fs::read_to_string(repo.root.join("notes.txt")).expect("still there"),
    "untracked\n"
  );
  assert_eq!(
    repo.git(&["status", "--porcelain", "--untracked-files=all"]),
    "M README.md\nA  staged.txt\n?? notes.txt",
    "uncommitted, staged and untracked as before"
  );
}

/// The refs of the repository, by name.
fn refs(repo: &Repo) -> String {
  repo.git(&["for-each-ref", "--format=%(refname)"])
}

/// Git lists only the checkout and the run's worktree, keeps metadata only of the run's, no
/// temporary folder is in the git folder, and the refs are `before`.
fn assert_nothing_left(repo: &Repo, before: &str) {
  let listed = repo.git(&["worktree", "list", "--porcelain"]);
  let worktrees = listed.lines().filter(|line| line.starts_with("worktree ")).count();
  assert_eq!(worktrees, 2, "the checkout and the run's worktree: {listed}");
  let names = |dir: &str| -> Vec<String> {
    fs::read_dir(repo.root.join(dir))
      .expect("a folder")
      .map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned())
      .collect()
  };
  assert_eq!(names(".git/worktrees"), ["run"]);
  assert!(
    names(".git").iter().all(|name| !name.starts_with("orchester-merge")),
    "{:?}",
    names(".git")
  );
  assert_eq!(refs(repo), before);
}

fn merge(repo: &Repo, run: &Run) -> Result<orchester_core::integration::RunMerge, MergeError> {
  merge_run(&Processes, &git_program(), &repo.root, (run, &task()), now())
}

#[test]
fn merge_commits_beside_uncommitted_work_it_does_not_reach() {
  let (repo, base, head) = repo_with_run_work();
  let moved = repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  leave_unrelated_work(&repo);
  let before = refs(&repo);
  let merged = merge(&repo, &run(&base, Some("main"), Ending::Succeeded)).expect("merged");
  assert_eq!(merged.kind, MergeKind::MergeCommit);
  assert_eq!(merged.revision.as_str(), repo.head());
  assert_eq!(repo.git(&["log", "-1", "--format=%s"]), "Merge ORC-12 run #3");
  assert_eq!(repo.git(&["log", "-1", "--format=%P"]), format!("{moved} {head}"));
  assert_eq!(repo.git(&["log", "-1", "--format=%an"]), "Orchester Test");
  assert_eq!(
    fs::read_to_string(repo.root.join("feature.txt")).expect("merged file"),
    "work\n"
  );
  assert_unrelated_work_kept(&repo);
  assert_nothing_left(&repo, &before);
}

#[test]
fn merge_fast_forwards_beside_uncommitted_work_it_does_not_reach() {
  let (repo, base, head) = repo_with_run_work();
  leave_unrelated_work(&repo);
  let before = refs(&repo);
  let merged = merge(&repo, &run(&base, Some("main"), Ending::Succeeded)).expect("merged");
  assert_eq!(merged.kind, MergeKind::FastForward);
  assert_eq!(repo.head(), head);
  assert!(repo.root.join("feature.txt").is_file());
  assert_unrelated_work_kept(&repo);
  assert_nothing_left(&repo, &before);
}

#[test]
fn merge_refuses_a_change_it_would_overwrite_and_names_only_that_file() {
  let repo = Repo::new();
  let base = repo.head();
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  repo.commit(&worktree, "README.md", "the run's line\n", "Implement");
  let moved = repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  leave_unrelated_work(&repo);
  let before = refs(&repo);
  let refused = merge(&repo, &run(&base, Some("main"), Ending::Succeeded));
  assert_eq!(
    refused,
    Err(MergeError::UncommittedInTheWay {
      files: vec!["README.md".to_owned()]
    }),
    "the staged and the untracked file are not in its way"
  );
  assert_eq!(repo.head(), moved);
  assert_unrelated_work_kept(&repo);
  assert_nothing_left(&repo, &before);
}

#[test]
fn merge_refuses_an_untracked_file_in_its_way_and_names_only_that_file() {
  let (repo, base, _) = repo_with_run_work();
  fs::write(repo.root.join("feature.txt"), "the user's own\n").expect("untracked in the way");
  fs::write(repo.root.join("notes.txt"), "untracked\n").expect("untracked beside");
  let before = refs(&repo);
  let refused = merge(&repo, &run(&base, Some("main"), Ending::Succeeded));
  assert_eq!(
    refused,
    Err(MergeError::UncommittedInTheWay {
      files: vec!["feature.txt".to_owned()]
    })
  );
  assert_eq!(repo.head(), base);
  assert_eq!(
    fs::read_to_string(repo.root.join("feature.txt")).expect("kept"),
    "the user's own\n"
  );
  assert!(repo.root.join("notes.txt").is_file());
  assert_nothing_left(&repo, &before);
}

#[test]
fn merge_leaves_the_checkout_as_it_was_after_a_conflict_beside_uncommitted_work() {
  let repo = Repo::new();
  let base = repo.head();
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  repo.commit(&worktree, "README.md", "the run's line\n", "Implement");
  let moved = repo.commit(&repo.root, "README.md", "the user's line\n", "the user's own work");
  fs::write(repo.root.join("notes.txt"), "untracked\n").expect("untracked file");
  let before = refs(&repo);
  let refused = merge(&repo, &run(&base, Some("main"), Ending::Succeeded));
  assert_eq!(
    refused,
    Err(MergeError::Conflict {
      files: vec!["README.md".to_owned()]
    })
  );
  assert_eq!(repo.head(), moved);
  assert_eq!(repo.git(&["status", "--porcelain"]), "?? notes.txt");
  assert!(!repo.root.join(".git").join("MERGE_HEAD").exists());
  assert_nothing_left(&repo, &before);
}

#[test]
fn merge_leaves_the_checkout_as_it_was_when_a_hook_refuses_beside_uncommitted_work() {
  let (repo, base, _) = repo_with_run_work();
  let moved = repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  let hook = repo.root.join(".git").join("hooks").join("pre-merge-commit");
  fs::write(&hook, "#!/bin/sh\necho 'merges are frozen' >&2\nexit 1\n").expect("hook");
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("executable hook");
  }
  leave_unrelated_work(&repo);
  let before = refs(&repo);
  let refused = merge(&repo, &run(&base, Some("main"), Ending::Succeeded));
  let Err(MergeError::Refused(said)) = refused else {
    panic!("the hook refuses the merge: {refused:?}");
  };
  assert!(said.contains("merges are frozen"), "git's message: {said}");
  assert_eq!(repo.head(), moved);
  assert_unrelated_work_kept(&repo);
  assert_nothing_left(&repo, &before);
}

#[test]
fn merge_removes_the_temporary_worktree_a_killed_merge_left() {
  for folder_kept in [true, false] {
    let (repo, base, _) = repo_with_run_work();
    repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
    let finished = run(&base, Some("main"), Ending::Succeeded);
    let common = repo.git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]);
    let left = format!("{common}/orchester-merge-{}", finished.id());
    repo.git(&["worktree", "add", "--quiet", "--detach", &left, &base]);
    fs::write(format!("{left}/half.txt"), "half merged\n").expect("a file the killed merge left");
    if !folder_kept {
      fs::remove_dir_all(&left).expect("the folder goes, git still lists it");
    }
    let before = refs(&repo);
    let merged = merge(&repo, &finished).expect("merged");
    assert_eq!(merged.kind, MergeKind::MergeCommit, "folder kept: {folder_kept}");
    assert_nothing_left(&repo, &before);
  }
}
