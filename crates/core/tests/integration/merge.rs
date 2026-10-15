//! Merging a run into the branch the checkout had when it started, against real repositories.

use std::fs;

use orchester_core::{
  integration::{MergeError, MergeKind, merge_run, merge_run_at},
  revision::Revision,
  run::BranchName,
};

use crate::support::{Ending, Processes, RUN_BRANCH, Repo, git_in, git_program, now, run, task};

/// A repository whose run branch, in its run worktree, has one commit on top of `main`.
fn repo_with_run_work() -> (Repo, String, String) {
  let repo = Repo::new();
  let base = repo.head();
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  let head = repo.commit(&worktree, "feature.txt", "work\n", "Implement: add feature");
  (repo, base, head)
}

#[test]
fn merge_fast_forwards_the_base_branch_when_it_did_not_move() {
  let (repo, base, head) = repo_with_run_work();
  let merge = merge_run(
    &Processes,
    &git_program(),
    &repo.root,
    (&run(&base, Some("main"), Ending::Succeeded), &task()),
    now(),
  )
  .expect("merged");
  assert_eq!(merge.kind, MergeKind::FastForward);
  assert_eq!(merge.revision.as_str(), head);
  assert_eq!(merge.into.as_str(), "main");
  assert_eq!(merge.merged_at, now());
  assert_eq!(repo.head(), head);
  assert_eq!(repo.git(&["symbolic-ref", "--short", "HEAD"]), "main");
  assert_eq!(
    fs::read_to_string(repo.root.join("feature.txt")).expect("merged file"),
    "work\n"
  );
  assert!(repo.is_clean());
}

#[test]
fn merge_commits_when_the_base_branch_moved_meanwhile() {
  let (repo, base, head) = repo_with_run_work();
  let moved = repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  let merge = merge_run(
    &Processes,
    &git_program(),
    &repo.root,
    (&run(&base, Some("main"), Ending::Succeeded), &task()),
    now(),
  )
  .expect("merged");
  assert_eq!(merge.kind, MergeKind::MergeCommit);
  assert_eq!(merge.revision.as_str(), repo.head());
  assert_eq!(repo.git(&["log", "-1", "--format=%s"]), "Merge ORC-12 run #3");
  assert_eq!(repo.git(&["log", "-1", "--format=%P"]), format!("{moved} {head}"));
  // The repository's git identity signs the merge.
  assert_eq!(repo.git(&["log", "-1", "--format=%an"]), "Orchester Test");
  assert!(repo.root.join("feature.txt").is_file() && repo.root.join("other.txt").is_file());
  assert!(repo.is_clean());
}

#[test]
fn merge_records_a_run_the_base_branch_already_has() {
  let (repo, base, head) = repo_with_run_work();
  repo.git(&["merge", "--quiet", "--ff-only", RUN_BRANCH]);
  let merge = merge_run(
    &Processes,
    &git_program(),
    &repo.root,
    (&run(&base, Some("main"), Ending::Succeeded), &task()),
    now(),
  )
  .expect("recorded");
  assert_eq!(merge.kind, MergeKind::AlreadyMerged);
  assert_eq!(merge.revision.as_str(), head);
}

#[test]
fn merge_refuses_a_checkout_with_uncommitted_changes_and_touches_nothing() {
  let (repo, base, _) = repo_with_run_work();
  let before = repo.head();
  fs::write(repo.root.join("README.md"), "edited, not committed\n").expect("edit");
  fs::write(repo.root.join("scratch.txt"), "untracked\n").expect("untracked file");
  let refused = merge_run(
    &Processes,
    &git_program(),
    &repo.root,
    (&run(&base, Some("main"), Ending::Succeeded), &task()),
    now(),
  );
  assert_eq!(
    refused,
    Err(MergeError::Uncommitted {
      files: vec!["README.md".to_owned(), "scratch.txt".to_owned()]
    })
  );
  assert_eq!(repo.head(), before);
  assert_eq!(
    fs::read_to_string(repo.root.join("README.md")).expect("still edited"),
    "edited, not committed\n"
  );
  assert!(repo.root.join("scratch.txt").is_file());
}

#[test]
fn merge_refuses_a_checkout_on_another_branch_or_a_detached_head() {
  let (repo, base, _) = repo_with_run_work();
  let finished = run(&base, Some("main"), Ending::Succeeded);
  repo.git(&["switch", "--quiet", "-c", "feature"]);
  let refused = merge_run(&Processes, &git_program(), &repo.root, (&finished, &task()), now());
  assert_eq!(
    refused,
    Err(MergeError::WrongBranch {
      expected: BranchName::new("main").expect("valid"),
      current: Some(BranchName::new("feature").expect("valid")),
    })
  );
  repo.git(&["switch", "--quiet", "--detach", "main"]);
  let refused = merge_run(&Processes, &git_program(), &repo.root, (&finished, &task()), now());
  assert!(matches!(refused, Err(MergeError::WrongBranch { current: None, .. })));
  assert_eq!(repo.git(&["rev-parse", "main"]), base);
}

#[test]
fn merge_undoes_a_conflict_and_names_the_files() {
  let repo = Repo::new();
  let base = repo.head();
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  repo.commit(&worktree, "README.md", "the run's line\n", "Implement");
  let before = repo.commit(&repo.root, "README.md", "the user's line\n", "the user's own work");
  let refused = merge_run(
    &Processes,
    &git_program(),
    &repo.root,
    (&run(&base, Some("main"), Ending::Succeeded), &task()),
    now(),
  );
  assert_eq!(
    refused,
    Err(MergeError::Conflict {
      files: vec!["README.md".to_owned()]
    })
  );
  assert_eq!(repo.head(), before);
  assert!(repo.is_clean(), "the merge was undone");
  assert!(!repo.root.join(".git").join("MERGE_HEAD").exists());
  assert_eq!(
    fs::read_to_string(repo.root.join("README.md")).expect("readme"),
    "the user's line\n"
  );
}

#[test]
fn merge_lets_the_repository_hooks_run_and_undoes_what_they_refuse() {
  let (repo, base, _) = repo_with_run_work();
  let before = repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  let hook = repo.root.join(".git").join("hooks").join("pre-merge-commit");
  fs::write(&hook, "#!/bin/sh\necho 'merges are frozen' >&2\nexit 1\n").expect("hook");
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("executable hook");
  }
  let refused = merge_run(
    &Processes,
    &git_program(),
    &repo.root,
    (&run(&base, Some("main"), Ending::Succeeded), &task()),
    now(),
  );
  let Err(MergeError::Refused(said)) = refused else {
    panic!("the hook refuses the merge: {refused:?}");
  };
  assert!(said.contains("merges are frozen"), "git's message: {said}");
  assert_eq!(repo.head(), before);
  assert!(repo.is_clean());
  assert!(!repo.root.join(".git").join("MERGE_HEAD").exists());
}

#[test]
fn merge_refuses_a_checkout_with_a_merge_in_progress() {
  let repo = Repo::new();
  let base = repo.head();
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  repo.commit(&worktree, "README.md", "the run's line\n", "Implement");
  repo.commit(&repo.root, "README.md", "the user's line\n", "mine");
  git_in(&repo.root, &["branch", "--quiet", "elsewhere", RUN_BRANCH]);
  // The user's own merge stopped at a conflict.
  let stopped = std::process::Command::new("git")
    .arg("-C")
    .arg(&repo.root)
    .args(["merge", "--quiet", "elsewhere"])
    .output()
    .expect("run git");
  assert!(!stopped.status.success());
  let refused = merge_run(
    &Processes,
    &git_program(),
    &repo.root,
    (&run(&base, Some("main"), Ending::Succeeded), &task()),
    now(),
  );
  assert_eq!(refused, Err(MergeError::InProgress));
  assert!(
    repo.root.join(".git").join("MERGE_HEAD").exists(),
    "the user's merge is left alone"
  );
}

#[test]
fn merge_needs_a_run_that_succeeded_on_a_recorded_branch_that_still_exists() {
  let (repo, base, _) = repo_with_run_work();
  let merge = |run| merge_run(&Processes, &git_program(), &repo.root, (&run, &task()), now());
  assert_eq!(
    merge(run(&base, Some("main"), Ending::Cancelled)),
    Err(MergeError::NotSucceeded)
  );
  assert_eq!(
    merge(run(&base, None, Ending::Succeeded)),
    Err(MergeError::NoBaseBranch)
  );
  repo.git(&["worktree", "remove", "--force", ".orchester/worktrees/run"]);
  repo.git(&["branch", "--quiet", "-D", RUN_BRANCH]);
  assert_eq!(
    merge(run(&base, Some("main"), Ending::Succeeded)),
    Err(MergeError::BranchMissing(BranchName::new(RUN_BRANCH).expect("valid")))
  );
}

#[test]
fn merge_at_a_reviewed_commit_refuses_a_branch_that_moved_since() {
  let (repo, base, reviewed) = repo_with_run_work();
  let worktree = repo.root.join(".orchester").join("worktrees").join("run");
  let later = repo.commit(&worktree, "later.txt", "unreviewed\n", "Implement: more work");
  let expected = Revision::new(&reviewed).expect("a commit");
  let refused = merge_run_at(
    &Processes,
    (&git_program(), &repo.root),
    (&run(&base, Some("main"), Ending::Succeeded), &task()),
    Some(&expected),
    now(),
  );
  assert_eq!(
    refused,
    Err(MergeError::BranchMoved {
      branch: BranchName::new(RUN_BRANCH).expect("valid"),
      expected,
    }),
    "the branch moved from the reviewed {reviewed} to {later}"
  );
  assert_eq!(repo.head(), base, "nothing was merged");

  let at_head = Revision::new(&later).expect("a commit");
  let merged = merge_run_at(
    &Processes,
    (&git_program(), &repo.root),
    (&run(&base, Some("main"), Ending::Succeeded), &task()),
    Some(&at_head),
    now(),
  )
  .expect("merged at the commit asked for");
  assert_eq!(merged.revision.as_str(), later);
}
