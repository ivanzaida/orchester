use std::{fs, path::Path, process::Command};

use orchester_core::{
  id::{RunId, StepExecutionId},
  run::WorktreeFailure,
};
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

/// A `post-checkout` hook that prints a URL with credentials and fails. Git for Windows runs it with its own `sh`,
/// as Unix does.
fn failing_post_checkout_hook(root: &Path) {
  let hook = root.join(".git/hooks/post-checkout");
  fs::create_dir_all(hook.parent().expect("hooks folder")).expect("hooks folder");
  fs::write(
    &hook,
    "#!/bin/sh\necho 'fatal: cannot fetch https://someone:hunter2@example.com/lfs' >&2\nexit 1\n",
  )
  .expect("write hook");
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("make the hook executable");
  }
}

#[test]
fn a_worktree_whose_post_checkout_hook_fails_is_removed_before_its_branch() {
  let repo = repository();
  let root = repo.path();
  failing_post_checkout_hook(root);
  let worktrees = Worktrees::new(root).expect("worktrees");
  let run = RunId::generate();

  let error = worktrees
    .create(run, "orchester/run-1", "main")
    .expect_err("git reports the hook's failure");

  let WorktreeFailure::Git(said) = error.failure() else {
    panic!("git's refusal is the reason, got {error:?}");
  };
  assert!(said.as_str().contains("https://***@example.com/lfs"), "{said:?}");
  assert!(!said.as_str().contains("hunter2"), "no credentials: {said:?}");
  assert!(!worktrees.path(run).exists(), "the worktree git kept is gone");
  assert!(
    !git(root, &["worktree", "list", "--porcelain"]).contains("orchester/run-1"),
    "no worktree has the branch"
  );
  assert_eq!(
    git(root, &["branch", "--list", "orchester/run-1"]),
    "",
    "the new branch is gone once no worktree has it"
  );
}

#[test]
fn a_name_whose_step_branches_or_ref_lock_exist_is_taken() {
  let repo = repository();
  let root = repo.path();
  let worktrees = Worktrees::new(root).expect("worktrees");
  // An earlier run's leftover step branch: steps of a run on that name would need it.
  git(root, &["branch", "orchester/run-1--review-1"]);

  let worktree = worktrees
    .create(RunId::generate(), "orchester/run-1", "main")
    .expect("create");

  assert_eq!(worktree.branch(), "orchester/run-1-2");
  // Another creator holds the lock of the next name while it makes or deletes that branch.
  let lock = root.join(".git/refs/heads/orchester/run-1-3.lock");
  fs::write(&lock, "").expect("hold the ref's lock");
  git(root, &["branch", "orchester/run-1-2--work-1", "orchester/run-1-2"]);

  let worktree = worktrees
    .create(RunId::generate(), "orchester/run-1", "main")
    .expect("create");

  assert_eq!(
    worktree.branch(),
    "orchester/run-1-4",
    "a held lock is a name taken meanwhile"
  );
  assert!(lock.exists(), "the other creator's lock is left alone");
}

#[test]
fn a_run_worktree_fails_when_every_numbered_name_is_taken() {
  let repo = repository();
  let root = repo.path();
  let main = git(root, &["rev-parse", "main"]);
  let created: String = std::iter::once("orchester/run-1".to_owned())
    .chain((2..=100).map(|suffix| format!("orchester/run-1-{suffix}")))
    .map(|branch| format!("create refs/heads/{branch} {main}\n"))
    .collect();
  let mut update = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(["update-ref", "--stdin"])
    .stdin(std::process::Stdio::piped())
    .spawn()
    .expect("run git");
  std::io::Write::write_all(&mut update.stdin.take().expect("stdin"), created.as_bytes()).expect("send refs");
  assert!(update.wait().expect("git ends").success(), "every name taken");
  let worktrees = Worktrees::new(root).expect("worktrees");
  let run = RunId::generate();

  let error = worktrees
    .create(run, "orchester/run-1", "main")
    .expect_err("no free name");

  assert!(matches!(error, WorktreeError::NoFreeBranch(_)), "{error:?}");
  assert_eq!(error.failure(), WorktreeFailure::NoFreeBranch);
  assert!(!worktrees.path(run).exists());
}

#[test]
fn a_folder_git_does_not_list_is_not_the_runs_worktree() {
  let repo = repository();
  let worktrees = Worktrees::new(repo.path()).expect("worktrees");
  let run = RunId::generate();
  // Left by a removal that failed part way: git would answer for the checkout around it, on `main`.
  fs::create_dir_all(worktrees.path(run)).expect("stray folder");

  let error = worktrees.branch_of(run).expect_err("not a worktree");

  assert!(matches!(error, WorktreeError::NotAWorktree(_)), "{error:?}");
  assert_eq!(error.failure(), WorktreeFailure::FolderInTheWay);
}
