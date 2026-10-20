//! Where a run's own work starts (ORC-60), against real temporary git repositories: above the commits of main its
//! branch fast-forwarded onto or merged in, and at the revision it started from when main holds a commit of the run's
//! own; the reflog entries that made a commit on the branch.

use orchester_core::{
  integration::{RunWork, made_on_branch, own_base},
  revision::Revision,
  run::BranchName,
};

use super::support::{Processes, RUN_BRANCH, Repo, git_in, git_program};

fn work(base: &str) -> RunWork {
  RunWork {
    branch: BranchName::new(RUN_BRANCH).expect("branch"),
    base: Revision::new(base).expect("revision"),
    into: Some(BranchName::new("main").expect("branch")),
  }
}

fn revision(sha: &str) -> Revision {
  Revision::new(sha).expect("revision")
}

fn base_of(repo: &Repo, (base, head): (&str, &str), checkpoints: &[Revision]) -> String {
  own_base(
    &Processes,
    (&git_program(), &repo.root),
    (&work(base), &revision(head)),
    (checkpoints, None),
  )
  .expect("read")
  .to_string()
}

#[test]
fn fork_a_branch_fast_forwarded_onto_main_starts_at_the_commit_it_took_in() {
  let repo = Repo::new();
  let base = repo.head();
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  let landed = repo.commit(&repo.root, "landed.txt", "landed\n", "ORC-51: landed");
  git_in(&worktree, &["merge", "main"]);
  let own = repo.commit(&worktree, "task.txt", "task\n", "ORC-12: Implement (run #3)");
  repo.commit(&repo.root, "later.txt", "later\n", "later main work");
  assert_eq!(base_of(&repo, (&base, &own), &[revision(&own)]), landed);
}

#[test]
fn fork_a_checkpoint_main_holds_keeps_the_start() {
  let repo = Repo::new();
  let base = repo.head();
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  let first = repo.commit(&worktree, "task.txt", "task\n", "task work");
  let more = repo.commit(&worktree, "more.txt", "more\n", "more task work");
  git_in(&repo.root, &["merge", "--no-ff", "--no-edit", &first]);
  let checkpoints = [revision(&first), revision(&more)];
  assert_eq!(base_of(&repo, (&base, &more), &checkpoints), base);
}

#[test]
fn fork_reflog_entries_that_made_a_commit_on_the_branch() {
  for made in [
    "commit: ORC-12: Implement (run #3)",
    "commit (amend): fix",
    "commit (merge): Merge main",
    "merge main: Merge made by the 'ort' strategy.",
    "cherry-pick: task work",
    "revert: Revert \"task work\"",
    "rebase (finish): refs/heads/task onto 1234",
  ] {
    assert!(made_on_branch(made), "{made}");
  }
  for moved in [
    "branch: Created from main",
    "merge main: Fast-forward",
    "pull: Fast-forward",
    "reset: moving to HEAD~1",
  ] {
    assert!(!made_on_branch(moved), "{moved}");
  }
}
