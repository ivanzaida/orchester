//! A run's base branch (ORC-81) against real repositories: the branches it can be set to, a remote-tracking one
//! creating its local branch, the tip a new run starts from on a branch set and a branch set that is gone, and a run
//! merged into a base branch the checkout does not have checked out: beside the checkout, which is neither switched
//! nor touched, never into a base checked out in another worktree, and, for a run that took the checked-out branch,
//! still only in the checkout.
//! `cargo test -p orchester-core --test integration base_branch`.

use std::fs;

use orchester_core::{
  integration::{
    DeliveredAs, Delivering, MergeError, MergeKind,
    base::{Landing, RunBaseError, branch_choices, create_tracking_branch, set_branch_tip},
    merge_run_as,
  },
  revision::Revision,
  run::BranchName,
};

use crate::support::{Ending, Processes, RUN_BRANCH, Repo, git_program, now, run, task};

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("valid branch")
}

/// A repository on `main` with a `staging` branch one commit ahead, whose run branch, in its run worktree, has one
/// commit on top of `staging`; the checkout stays on `main` with uncommitted work of the person's. The staging tip the
/// run started from, and the run's head.
fn repo_on_main_with_staging_run() -> (Repo, String, String) {
  let repo = Repo::new();
  repo.git(&["switch", "--quiet", "-c", "staging"]);
  let staging = repo.commit(&repo.root, "staging.txt", "staging\n", "staging work");
  repo.git(&["switch", "--quiet", "main"]);
  let worktree = repo.worktree("run", RUN_BRANCH, &staging);
  let head = repo.commit(&worktree, "feature.txt", "work\n", "Implement: add feature");
  fs::write(repo.root.join("README.md"), "the person's edit\n").expect("edit");
  (repo, staging, head)
}

fn merge_beside(
  repo: &Repo,
  base: &str,
  delivering: Option<Delivering<'_>>,
) -> Result<orchester_core::integration::RunMerge, MergeError> {
  merge_run_as(
    &Processes,
    (&git_program(), &repo.root),
    (&run(base, Some("staging"), Ending::Succeeded), &task()),
    (None, delivering, Landing::BesideOtherBranch),
    now(),
  )
  .map(|outcome| {
    assert_eq!(outcome.left, None, "the temporary worktree went");
    outcome.merge
  })
}

/// The checkout is on `main` at `main_head` with the person's edit, as before the merge.
fn assert_checkout_untouched(repo: &Repo, main_head: &str) {
  assert_eq!(repo.git(&["symbolic-ref", "--short", "HEAD"]), "main");
  assert_eq!(repo.head(), main_head);
  assert_eq!(repo.git(&["status", "--porcelain"]), "M README.md");
  assert!(
    !repo.root.join("feature.txt").exists(),
    "the run's file is not in the checkout"
  );
}

#[test]
fn base_branch_choices_are_local_branches_and_remote_tracking_ones_without_a_local_branch() {
  let repo = Repo::new();
  let head = repo.head();
  repo.git(&["branch", "staging"]);
  repo.git(&["remote", "add", "origin", "https://example.invalid/kontur.git"]);
  for name in ["main", "release", "feature/login"] {
    repo.git(&["update-ref", &format!("refs/remotes/origin/{name}"), &head]);
  }
  repo.git(&["symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main"]);
  let choices = branch_choices(&Processes, &git_program(), &repo.root).expect("choices");
  assert_eq!(choices.local, [branch("main"), branch("staging")]);
  let remote: Vec<(&str, &str)> = choices
    .remote
    .iter()
    .map(|remote| (remote.tracking(), remote.local().as_str()))
    .collect();
  assert_eq!(
    remote,
    [("origin/feature/login", "feature/login"), ("origin/release", "release")],
    "origin/main has its local branch and origin/HEAD is no branch"
  );
}

#[test]
fn base_branch_a_remote_tracking_choice_creates_its_local_branch_and_leaves_the_checkout() {
  let repo = Repo::new();
  let head = repo.head();
  repo.git(&["remote", "add", "origin", "https://example.invalid/kontur.git"]);
  repo.git(&["update-ref", "refs/remotes/origin/release", &head]);
  let choices = branch_choices(&Processes, &git_program(), &repo.root).expect("choices");
  let release = choices.remote.first().expect("origin/release");
  create_tracking_branch(&Processes, &git_program(), (&repo.root, release)).expect("created");
  assert_eq!(repo.git(&["rev-parse", "release"]), head);
  assert_eq!(
    repo.git(&["rev-parse", "--abbrev-ref", "release@{upstream}"]),
    "origin/release"
  );
  assert_eq!(repo.git(&["symbolic-ref", "--short", "HEAD"]), "main");
  let choices = branch_choices(&Processes, &git_program(), &repo.root).expect("choices");
  assert!(choices.has_local(&branch("release")) && choices.remote.is_empty());
}

#[test]
fn base_branch_a_new_run_starts_from_the_tip_of_the_branch_set_and_a_gone_one_is_refused() {
  let (repo, staging, _) = repo_on_main_with_staging_run();
  let tip = set_branch_tip(&Processes, &git_program(), (&repo.root, &branch("staging"))).expect("tip");
  assert_eq!(tip.as_str(), staging);
  assert_ne!(tip.as_str(), repo.head(), "not the checkout's HEAD");
  assert_eq!(
    set_branch_tip(&Processes, &git_program(), (&repo.root, &branch("gone"))),
    Err(RunBaseError::Missing(branch("gone")))
  );
}

#[test]
fn base_branch_merge_fast_forwards_a_base_the_checkout_does_not_have_beside_it() {
  let (repo, staging, head) = repo_on_main_with_staging_run();
  let main_head = repo.head();
  let merge = merge_beside(&repo, &staging, None).expect("merged");
  assert_eq!(
    (merge.kind, merge.into.as_str(), merge.revision.as_str()),
    (MergeKind::FastForward, "staging", head.as_str())
  );
  assert_eq!(repo.git(&["rev-parse", "staging"]), head);
  assert_checkout_untouched(&repo, &main_head);
}

#[test]
fn base_branch_merge_commits_beside_the_checkout_when_the_base_moved() {
  let (repo, staging, head) = repo_on_main_with_staging_run();
  let main_head = repo.head();
  let worktree = repo.worktree("person", "elsewhere", &staging);
  let moved = repo.commit(&worktree, "other.txt", "theirs\n", "someone else's work");
  repo.git(&["worktree", "remove", worktree.to_str().expect("path")]);
  repo.git(&["update-ref", "refs/heads/staging", &moved]);
  let merge = merge_beside(&repo, &staging, None).expect("merged");
  assert_eq!(merge.kind, MergeKind::MergeCommit);
  assert_eq!(repo.git(&["rev-parse", "staging"]), merge.revision.as_str());
  assert_eq!(
    repo.git(&["log", "-1", "--format=%P", "staging"]),
    format!("{moved} {head}")
  );
  assert_eq!(
    repo.git(&["log", "-1", "--format=%s", "staging"]),
    "Merge ORC-12 run #3"
  );
  assert_checkout_untouched(&repo, &main_head);
}

#[test]
fn base_branch_merge_squashes_beside_the_checkout_as_the_queue_delivers() {
  let (repo, staging, _) = repo_on_main_with_staging_run();
  let main_head = repo.head();
  let at = Revision::new(&staging).expect("revision");
  let delivering = Delivering {
    as_delivered: DeliveredAs::Squash {
      title: "ORC-12: Add feature",
      body: "Delivered by Orchester's merge queue.",
    },
    base: &at,
  };
  let merge = merge_beside(&repo, &staging, Some(delivering)).expect("merged");
  assert_eq!(merge.kind, MergeKind::Squash);
  assert_eq!(repo.git(&["log", "-1", "--format=%P", "staging"]), staging);
  assert_eq!(
    repo.git(&["log", "-1", "--format=%s", "staging"]),
    "ORC-12: Add feature"
  );
  assert_checkout_untouched(&repo, &main_head);
  let other = Revision::new(&main_head).expect("revision");
  let stale = Delivering {
    as_delivered: DeliveredAs::MergeCommit,
    base: &other,
  };
  assert_eq!(
    merge_beside(&repo, &staging, Some(stale)),
    Err(MergeError::BaseMoved {
      branch: branch("staging")
    }),
    "a base no longer where it was checked merges nothing"
  );
}

#[test]
fn base_branch_merge_never_moves_a_base_checked_out_in_another_worktree() {
  let (repo, staging, _) = repo_on_main_with_staging_run();
  let main_head = repo.head();
  let worktree = repo.root.join(".orchester").join("staging");
  repo.git(&[
    "worktree",
    "add",
    "--quiet",
    worktree.to_str().expect("path"),
    "staging",
  ]);
  let refused = merge_beside(&repo, &staging, None);
  assert!(
    matches!(&refused, Err(MergeError::CheckedOutIn { branch: refused, .. }) if refused.as_str() == "staging"),
    "{refused:?}"
  );
  assert_eq!(repo.git(&["rev-parse", "staging"]), staging, "nothing moved");
  assert_checkout_untouched(&repo, &main_head);
}

#[test]
fn base_branch_merge_lands_in_the_checkout_while_it_has_the_base_and_follows_the_checkout_by_default() {
  let (repo, staging, head) = repo_on_main_with_staging_run();
  let following = merge_run_as(
    &Processes,
    (&git_program(), &repo.root),
    (&run(&staging, Some("staging"), Ending::Succeeded), &task()),
    (None, None, Landing::Checkout),
    now(),
  );
  assert!(
    matches!(following, Err(MergeError::WrongBranch { .. })),
    "a project that follows the checkout merges only there: {following:?}"
  );
  repo.git(&["checkout", "--quiet", "--", "README.md"]);
  repo.git(&["switch", "--quiet", "staging"]);
  let merge = merge_beside(&repo, &staging, None).expect("merged");
  assert_eq!(merge.kind, MergeKind::FastForward);
  assert_eq!(repo.head(), head, "the checkout on staging took the merge");
  assert!(repo.root.join("feature.txt").is_file());
}
