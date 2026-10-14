//! What became of a succeeded run's work that Orchester did not merge, against real repositories: merged by a
//! person, nothing to merge, or still waiting.

use orchester_core::{
  integration::{Branches, Delivered, RunWork},
  revision::Revision,
  run::BranchName,
};

use crate::support::{Processes, RUN_BRANCH, Repo, git_program};

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("a branch name")
}

/// The run on `RUN_BRANCH` that started at `base` from `into`.
fn work(base: &str, into: &str) -> RunWork {
  RunWork {
    branch: branch(RUN_BRANCH),
    base: Revision::new(base).expect("a revision"),
    into: Some(branch(into)),
  }
}

fn delivered(repo: &Repo, work: &RunWork) -> Delivered {
  let program = git_program();
  Branches::read(&Processes, &program, &repo.root)
    .expect("git lists the branches")
    .delivered(work)
    .expect("git answers")
}

#[test]
fn merged_only_once_the_base_holds_commits_the_run_made() {
  let repo = Repo::new();
  let base = repo.head();
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  repo.commit(&worktree, "feature.txt", "work\n", "Implement: add feature");
  assert_eq!(delivered(&repo, &work(&base, "main")), Delivered::Waiting);
  repo.git(&["merge", "--quiet", "--no-ff", "-m", "merged by hand", RUN_BRANCH]);
  assert_eq!(
    delivered(&repo, &work(&base, "main")),
    Delivered::Merged,
    "a merge made with git counts"
  );
}

#[test]
fn merged_a_run_that_committed_nothing_has_nothing_to_merge() {
  let repo = Repo::new();
  let base = repo.head();
  repo.worktree("run", RUN_BRANCH, &base);
  assert_eq!(
    delivered(&repo, &work(&base, "main")),
    Delivered::NothingToMerge,
    "its tip is where it started, which main holds: that is no merge"
  );
  repo.commit(&repo.root, "other.txt", "mine\n", "the person's own work");
  assert_eq!(delivered(&repo, &work(&base, "main")), Delivered::NothingToMerge);
}

#[test]
fn merged_a_run_branch_gone_back_before_its_start_has_nothing_to_merge() {
  let repo = Repo::new();
  let first = repo.head();
  let base = repo.commit(&repo.root, "second.txt", "two\n", "second");
  repo.worktree("run", RUN_BRANCH, &first);
  assert_eq!(delivered(&repo, &work(&base, "main")), Delivered::NothingToMerge);
}

#[test]
fn merged_without_its_branch_or_its_base_branch_waits() {
  let repo = Repo::new();
  let base = repo.head();
  assert_eq!(
    delivered(&repo, &work(&base, "main")),
    Delivered::Waiting,
    "no run branch"
  );
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  repo.commit(&worktree, "feature.txt", "work\n", "Implement: add feature");
  assert_eq!(
    delivered(&repo, &work(&base, "release")),
    Delivered::Waiting,
    "no base branch"
  );
}
