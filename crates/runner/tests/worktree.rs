use std::{fs, path::Path, process::Command};

use orchester_core::id::{RunId, StepExecutionId};
use orchester_runner::worktree::{RemoveMode, WorktreeError, Worktrees};
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) -> String {
  let output = Command::new("git")
    .arg("-C")
    .arg(dir)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?} failed: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A repository with one commit on `main` and `.orchester/` excluded, as project setup leaves it.
fn repository() -> TempDir {
  let repo = tempfile::tempdir().expect("temporary directory");
  let root = repo.path();
  git(root, &["init", "--quiet", "--initial-branch=main"]);
  git(root, &["config", "user.name", "Orchester Test"]);
  git(root, &["config", "user.email", "test@orchester.invalid"]);
  git(root, &["config", "core.autocrlf", "false"]);
  git(root, &["config", "commit.gpgsign", "false"]);
  fs::write(root.join("README.md"), "hello\n").expect("write file");
  git(root, &["add", "README.md"]);
  git(root, &["commit", "--quiet", "-m", "initial"]);
  fs::create_dir(root.join(".orchester")).expect("create .orchester");
  fs::write(root.join(".git/info/exclude"), ".orchester/\n").expect("exclude .orchester");
  repo
}

#[test]
fn worktree_is_created_on_a_new_branch_without_touching_the_checkout() {
  let repo = repository();
  let root = repo.path();
  let head = git(root, &["rev-parse", "HEAD"]);
  let worktrees = Worktrees::new(root).expect("worktrees");
  let run = RunId::generate();

  let worktree = worktrees.create(run, "orchester/run-1", "main").expect("create");

  assert_eq!(worktree.path(), worktrees.path(run));
  assert!(worktree.path().starts_with(root.join(".orchester").join("worktrees")));
  assert_eq!(worktree.branch(), "orchester/run-1");
  assert_eq!(worktree.base_revision(), head);
  assert_eq!(
    fs::read_to_string(worktree.path().join("README.md")).expect("checked out"),
    "hello\n"
  );
  assert_eq!(git(worktree.path(), &["branch", "--show-current"]), "orchester/run-1");
  assert_eq!(git(root, &["branch", "--show-current"]), "main");
  assert_eq!(git(root, &["status", "--porcelain", "--untracked-files=all"]), "");
}

#[test]
fn worktree_creation_rejects_bad_input() {
  let repo = repository();
  let worktrees = Worktrees::new(repo.path()).expect("worktrees");

  assert!(matches!(
    worktrees.create_step(StepExecutionId::generate(), "main", "main"),
    Err(WorktreeError::BranchExists(_))
  ));
  assert!(matches!(
    worktrees.create(RunId::generate(), "bad..name", "main"),
    Err(WorktreeError::InvalidBranch(_))
  ));
  assert!(matches!(
    worktrees.create(RunId::generate(), "orchester/run-2", "no-such-branch"),
    Err(WorktreeError::UnknownBase(_))
  ));

  let run = RunId::generate();
  worktrees.create(run, "orchester/run-3", "main").expect("create");
  assert!(matches!(
    worktrees.create(run, "orchester/run-4", "main"),
    Err(WorktreeError::AlreadyExists(_))
  ));
}

#[test]
fn a_run_worktree_takes_the_next_free_branch_name_and_leaves_taken_ones() {
  let repo = repository();
  let root = repo.path();
  let main = git(root, &["rev-parse", "main"]);
  git(root, &["branch", "main-2"]);
  let worktrees = Worktrees::new(root).expect("worktrees");
  let run = RunId::generate();

  let worktree = worktrees.create(run, "main", "main").expect("create");

  assert_eq!(worktree.branch(), "main-3");
  assert_eq!(git(worktree.path(), &["branch", "--show-current"]), "main-3");
  assert_eq!(worktrees.branch_of(run).expect("branch of the worktree"), "main-3");
  assert_eq!(git(root, &["rev-parse", "main"]), main);
  assert_eq!(git(root, &["rev-parse", "main-2"]), main);
  assert_eq!(git(root, &["branch", "--show-current"]), "main");
}

#[test]
fn a_worktree_that_cannot_be_added_leaves_no_branch_behind() {
  let repo = repository();
  let root = repo.path();
  let worktrees = Worktrees::new(root).expect("worktrees");
  // A file where git keeps its worktree records makes `git worktree add` fail after `git branch`.
  fs::write(root.join(".git/worktrees"), "not a folder").expect("block git's worktree records");

  assert!(matches!(
    worktrees.create(RunId::generate(), "orchester/run-1", "main"),
    Err(WorktreeError::Git { .. })
  ));
  assert_eq!(
    git(root, &["branch", "--list", "orchester/run-1"]),
    "",
    "the new branch is gone"
  );
}

#[test]
fn worktree_creation_requires_orchester_to_be_excluded() {
  let repo = repository();
  fs::write(repo.path().join(".git/info/exclude"), "").expect("clear exclude");
  let worktrees = Worktrees::new(repo.path()).expect("worktrees");

  assert!(matches!(
    worktrees.create(RunId::generate(), "orchester/run-1", "main"),
    Err(WorktreeError::NotExcluded(_))
  ));
}

#[test]
fn worktree_removal_keeps_uncommitted_work_unless_forced() {
  let repo = repository();
  let worktrees = Worktrees::new(repo.path()).expect("worktrees");
  let run = RunId::generate();
  let worktree = worktrees.create(run, "orchester/run-1", "main").expect("create");
  let work = worktree.path().join("work.txt");
  fs::write(&work, "unsaved\n").expect("write work");

  assert!(matches!(
    worktrees.remove(run, RemoveMode::RefuseIfDirty),
    Err(WorktreeError::Dirty { changes: 1, .. })
  ));
  assert!(work.exists(), "refused removal must keep the work");

  worktrees
    .remove(run, RemoveMode::DiscardChanges)
    .expect("forced removal");
  assert!(!worktree.path().exists());
  assert_eq!(
    git(repo.path(), &["branch", "--list", "orchester/run-1"]),
    "orchester/run-1",
    "the run's branch is kept"
  );
}

#[test]
fn clean_worktree_is_removed_and_its_commits_stay_on_the_branch() {
  let repo = repository();
  let worktrees = Worktrees::new(repo.path()).expect("worktrees");
  let run = RunId::generate();
  let worktree = worktrees.create(run, "orchester/run-1", "main").expect("create");
  fs::write(worktree.path().join("work.txt"), "saved\n").expect("write work");
  git(worktree.path(), &["add", "work.txt"]);
  git(worktree.path(), &["commit", "--quiet", "-m", "agent work"]);
  let committed = git(worktree.path(), &["rev-parse", "HEAD"]);

  worktrees.remove(run, RemoveMode::RefuseIfDirty).expect("remove");

  assert!(!worktree.path().exists());
  assert_eq!(git(repo.path(), &["rev-parse", "orchester/run-1"]), committed);
  assert!(matches!(
    worktrees.remove(run, RemoveMode::RefuseIfDirty),
    Err(WorktreeError::NotFound(_))
  ));
}
