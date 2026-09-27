//! What a run leaves in the repository and clearing it, against real repositories: its worktree
//! and a kept step worktree, merged and unmerged branches, uncommitted changes, and a worktree whose
//! folder is gone.

use std::fs;

use orchester_core::{
  integration::{CleanupError, KeepReason, WorktreeOwner, clean_up, leftovers, merge_run},
  run::{BranchName, Run},
};

use crate::support::{Ending, Processes, RUN_BRANCH, Repo, execution, git_program, now, run, task};

/// The branch of the Review step of attempt 1, which ran beside Checks and failed.
const STEP_BRANCH: &str = "orchester/orc-12-run-3--review-1";

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("valid branch")
}

/// A succeeded run with its worktree and a kept worktree of its Review step whose commit was not
/// merged anywhere.
fn repo_with_leftovers(ending: Ending) -> (Repo, Run) {
  let repo = Repo::new();
  let base = repo.head();
  let run = run(&base, Some("main"), ending);
  let worktree = repo.worktree(&run.id().to_string(), RUN_BRANCH, &base);
  repo.commit(&worktree, "feature.txt", "work\n", "Implement");
  let review = execution(&run, "review").to_string();
  let step = repo.worktree(&review, STEP_BRANCH, RUN_BRANCH);
  repo.commit(&step, "review.txt", "notes\n", "Review: failed attempt");
  (repo, run)
}

#[test]
fn leftovers_list_the_worktrees_their_changes_and_which_branches_are_merged() {
  let (repo, run) = repo_with_leftovers(Ending::Succeeded);
  // A worktree of someone else's is not the run's.
  repo.worktree("unrelated", "orchester/orc-99-run-1", "main");
  fs::write(
    repo
      .root
      .join(".orchester/worktrees")
      .join(run.id().to_string())
      .join("draft.txt"),
    "not committed\n",
  )
  .expect("write");
  let left = leftovers(&Processes, &git_program(), &repo.root, &run).expect("read");
  assert_eq!(left.worktrees.len(), 2);
  let own = &left.worktrees[0];
  assert_eq!(own.owner, WorktreeOwner::Run);
  assert_eq!(own.branch, Some(branch(RUN_BRANCH)));
  assert_eq!(own.uncommitted, ["draft.txt"]);
  assert_eq!(left.worktrees[1].owner, WorktreeOwner::Step(execution(&run, "review")));
  assert!(left.worktrees[1].uncommitted.is_empty());
  assert_eq!(left.uncommitted(), 1);
  let branches: Vec<(&str, bool)> = left
    .branches
    .iter()
    .map(|branch| (branch.name.as_str(), branch.merged))
    .collect();
  assert_eq!(branches, [(RUN_BRANCH, false), (STEP_BRANCH, false)]);
}

#[test]
fn clean_up_after_a_merge_removes_the_worktrees_and_the_merged_branch_and_keeps_the_unmerged_one() {
  let (repo, run) = repo_with_leftovers(Ending::Succeeded);
  merge_run(&Processes, &git_program(), &repo.root, (&run, &task()), now()).expect("merged");
  let cleanup = clean_up(&Processes, &git_program(), &repo.root, &run, None).expect("cleared");
  assert_eq!(cleanup.removed.len(), 2);
  assert!(cleanup.kept_worktrees.is_empty());
  assert_eq!(cleanup.deleted, [branch(RUN_BRANCH)]);
  assert_eq!(cleanup.kept_branches, [(branch(STEP_BRANCH), KeepReason::NotMerged)]);
  assert!(
    !repo
      .root
      .join(".orchester/worktrees")
      .join(run.id().to_string())
      .exists()
  );
  assert!(!repo.branch_exists(RUN_BRANCH));
  assert!(
    repo.branch_exists(STEP_BRANCH),
    "the failed step's commits stay on its branch"
  );
  assert!(
    repo.root.join("feature.txt").is_file(),
    "the merged work is in the checkout"
  );
  let left = leftovers(&Processes, &git_program(), &repo.root, &run).expect("read");
  assert!(left.worktrees.is_empty());
  assert!(
    left.branches.iter().all(|branch| !branch.merged),
    "only the unmerged step branch is left"
  );
}

#[test]
fn clean_up_keeps_a_worktree_with_uncommitted_changes_until_they_are_confirmed() {
  let (repo, run) = repo_with_leftovers(Ending::Cancelled);
  let own = repo.root.join(".orchester/worktrees").join(run.id().to_string());
  fs::write(own.join("draft.txt"), "not committed\n").expect("write");
  let cleanup = clean_up(&Processes, &git_program(), &repo.root, &run, None).expect("cleared");
  assert_eq!(cleanup.removed.len(), 1, "the clean step worktree goes");
  let [(kept, reason)] = cleanup.kept_worktrees.as_slice() else {
    panic!("the run's worktree is kept: {cleanup:?}");
  };
  assert_eq!(kept.owner, WorktreeOwner::Run);
  assert_eq!(reason, &KeepReason::Uncommitted(vec!["draft.txt".to_owned()]));
  assert!(own.join("draft.txt").is_file());
  assert!(repo.branch_exists(RUN_BRANCH), "an unmerged run branch is kept");
  // What a person saw before confirming; then another file appears, which nobody confirmed.
  let seen = leftovers(&Processes, &git_program(), &repo.root, &run).expect("read");
  fs::write(own.join("later.txt"), "written after the dialog\n").expect("write");
  let cleanup = clean_up(&Processes, &git_program(), &repo.root, &run, Some(&seen)).expect("cleared");
  assert!(cleanup.removed.is_empty());
  assert!(own.join("later.txt").is_file());
  // Confirmed with exactly what is there now: removed, with the changes.
  let seen = leftovers(&Processes, &git_program(), &repo.root, &run).expect("read");
  let cleanup = clean_up(&Processes, &git_program(), &repo.root, &run, Some(&seen)).expect("cleared");
  assert_eq!(cleanup.removed.len(), 1);
  assert!(!own.exists());
  assert_eq!(cleanup.kept_branches.len(), 2, "unmerged branches stay: {cleanup:?}");
}

#[test]
fn clean_up_copes_with_a_worktree_whose_folder_is_gone() {
  let (repo, run) = repo_with_leftovers(Ending::Succeeded);
  merge_run(&Processes, &git_program(), &repo.root, (&run, &task()), now()).expect("merged");
  fs::remove_dir_all(repo.root.join(".orchester/worktrees").join(run.id().to_string())).expect("delete folder");
  let left = leftovers(&Processes, &git_program(), &repo.root, &run).expect("read");
  assert!(left.worktrees[0].missing);
  let cleanup = clean_up(&Processes, &git_program(), &repo.root, &run, None).expect("cleared");
  assert_eq!(cleanup.removed.len(), 2);
  assert_eq!(cleanup.deleted, [branch(RUN_BRANCH)]);
  assert!(
    repo
      .git(&["worktree", "list", "--porcelain"])
      .lines()
      .filter(|line| line.starts_with("worktree "))
      .count()
      == 1
  );
}

#[test]
fn clean_up_leaves_a_run_that_has_not_ended_alone() {
  let (repo, run) = repo_with_leftovers(Ending::Running);
  assert_eq!(
    clean_up(&Processes, &git_program(), &repo.root, &run, None),
    Err(CleanupError::NotFinished)
  );
  assert!(
    repo
      .root
      .join(".orchester/worktrees")
      .join(run.id().to_string())
      .is_dir()
  );
}
