//! Resolving a merge's conflicts by hand in the run's worktree, against real repositories: the base branch is merged
//! into the run's branch there with git's markers, *Continue* commits it only once no marker is left, *Abort* puts
//! the worktree back, and a worktree with uncommitted work is not merged into.

use std::fs;

use orchester_core::{
  integration::resolve::{self, ResolveError, Started, holds_markers},
  run::BranchName,
};

use crate::support::{Processes, RUN_BRANCH, Repo, git_in, git_program};

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("a branch name")
}

/// A run worktree and `main` that both appended a line to the end of `list.txt` since the run started.
fn conflicting() -> (Repo, std::path::PathBuf) {
  let repo = Repo::new();
  repo.commit(&repo.root, "list.txt", "first\n", "a list");
  let base = repo.head();
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  repo.commit(
    &worktree,
    "list.txt",
    "first\nrun's\n",
    "Implement: append the run's entry",
  );
  repo.commit(&repo.root, "list.txt", "first\nmain's\n", "append main's entry");
  (repo, worktree)
}

fn start(worktree: &std::path::Path) -> Result<Started, ResolveError> {
  resolve::start(
    &Processes,
    &git_program(),
    worktree,
    (&branch("main"), &branch(RUN_BRANCH)),
  )
}

#[test]
fn resolve_merges_the_base_into_the_worktree_and_leaves_the_markers() {
  let (repo, worktree) = conflicting();
  let checkout = repo.head();
  assert_eq!(start(&worktree), Ok(Started::Conflicts(vec!["list.txt".to_owned()])));
  let text = fs::read_to_string(worktree.join("list.txt")).expect("the conflicted file");
  assert!(holds_markers(&text), "{text}");
  let resolution = resolve::resolution(&Processes, &git_program(), &worktree)
    .expect("read")
    .expect("a merge in progress");
  assert_eq!(resolution.conflicted, ["list.txt"]);
  assert_eq!(resolution.markers, ["list.txt"]);
  // The person's checkout is not touched.
  assert_eq!(repo.head(), checkout);
  assert!(repo.is_clean());
  assert_eq!(start(&worktree), Err(ResolveError::InMerge));
}

#[test]
fn resolve_continue_refuses_while_markers_remain_and_commits_once_resolved() {
  let (repo, worktree) = conflicting();
  start(&worktree).expect("merged with conflicts");
  assert_eq!(
    resolve::finish(&Processes, &git_program(), &worktree),
    Err(ResolveError::MarkersLeft(vec!["list.txt".to_owned()]))
  );
  fs::write(worktree.join("list.txt"), "first\nmain's\nrun's\n").expect("resolved by hand");
  let resolution = resolve::resolution(&Processes, &git_program(), &worktree)
    .expect("read")
    .expect("still in the merge");
  assert_eq!(resolution.conflicted, ["list.txt"]);
  assert!(
    resolution.markers.is_empty(),
    "a resolved file holds no marker, staged or not"
  );
  let merged = resolve::finish(&Processes, &git_program(), &worktree).expect("committed");
  assert_eq!(merged.base.as_str(), repo.head());
  assert_eq!(merged.tip.as_str(), git_in(&worktree, &["rev-parse", "HEAD"]));
  assert_eq!(resolve::resolution(&Processes, &git_program(), &worktree), Ok(None));
  assert_eq!(git_in(&worktree, &["status", "--porcelain"]), "");
  // The run's branch holds main now, so main merges into it without a conflict.
  let main = repo.head();
  git_in(&worktree, &["merge-base", "--is-ancestor", &main, "HEAD"]);
  assert_eq!(
    git_in(&worktree, &["log", "-1", "--format=%s"]),
    format!("Merge main into {RUN_BRANCH}")
  );
}

#[test]
fn resolve_abort_puts_the_worktree_back() {
  let (_repo, worktree) = conflicting();
  let before = git_in(&worktree, &["rev-parse", "HEAD"]);
  start(&worktree).expect("merged with conflicts");
  resolve::abort(&Processes, &git_program(), &worktree).expect("aborted");
  assert_eq!(git_in(&worktree, &["rev-parse", "HEAD"]), before);
  assert_eq!(git_in(&worktree, &["status", "--porcelain"]), "");
  assert_eq!(
    resolve::abort(&Processes, &git_program(), &worktree),
    Err(ResolveError::NotInMerge)
  );
}

#[test]
fn resolve_refuses_a_worktree_with_uncommitted_work() {
  let (_repo, worktree) = conflicting();
  fs::write(worktree.join("notes.txt"), "the agent's\n").expect("uncommitted work");
  assert_eq!(
    start(&worktree),
    Err(ResolveError::Uncommitted(vec!["notes.txt".to_owned()]))
  );
  assert_eq!(resolve::resolution(&Processes, &git_program(), &worktree), Ok(None));
}

#[test]
fn resolve_a_merge_without_conflicts_is_committed_at_once() {
  let repo = Repo::new();
  let base = repo.head();
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  repo.commit(&worktree, "feature.txt", "work\n", "Implement: add feature");
  repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  assert!(matches!(start(&worktree), Ok(Started::Merged(_))));
  assert!(worktree.join("other.txt").is_file());
}

#[test]
fn resolve_markers_are_told_at_the_start_of_a_line_only() {
  assert!(holds_markers("a\n<<<<<<< HEAD\nb\n=======\nc\n>>>>>>> main\n"));
  assert!(!holds_markers("Title\n=======\n\nText with <<<<<<< inside.\n"));
}
