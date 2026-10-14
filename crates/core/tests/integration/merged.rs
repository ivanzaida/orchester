//! Telling the branches a person merged themselves, against real repositories.

use orchester_core::{integration::merged_into, run::BranchName};

use crate::support::{Processes, RUN_BRANCH, Repo, git_program};

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("a branch name")
}

#[test]
fn merged_lists_a_run_branch_once_its_base_holds_its_work() {
  let repo = Repo::new();
  let base = repo.head();
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  repo.commit(&worktree, "feature.txt", "work\n", "Implement: add feature");
  let merged = || merged_into(&Processes, &git_program(), &repo.root, &branch("main")).expect("git lists the branches");
  assert!(!merged().contains(&branch(RUN_BRANCH)), "its commit is not on main yet");
  assert!(merged().contains(&branch("main")));

  repo.git(&["merge", "--quiet", "--no-ff", "-m", "merged by hand", RUN_BRANCH]);
  assert!(merged().contains(&branch(RUN_BRANCH)), "a merge made with git counts");
}

#[test]
fn merged_into_a_branch_that_is_gone_lists_nothing() {
  let repo = Repo::new();
  let merged = merged_into(&Processes, &git_program(), &repo.root, &branch("release")).expect("git answers");
  assert!(merged.is_empty());
}
