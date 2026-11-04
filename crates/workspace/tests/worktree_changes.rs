//! What a run's worktree holds beyond its commit, read through the project's real `orchester-runner`, which the tests
//! build themselves (`support/runner_programs.rs`): the same files and diff git reads in the worktree, the runner
//! started for the read when it is not running, and an explicit error, never what was read before, when it cannot
//! start. Needs `git`.

#[path = "support/runner_programs.rs"]
mod runner_programs;

use std::{
  fs,
  hash::{DefaultHasher, Hash, Hasher},
  path::Path,
  process::Command,
  sync::Arc,
  time::Duration,
};

use orchester_core::{
  diff::{
    Diff,
    git::{WorktreeRead, read_worktree_changes},
  },
  id::RunId,
};
use orchester_exec::ProcessRunner;
use orchester_ipc::SessionEnd::Clean;
use orchester_workspace::{
  activity::Activity,
  runner::{QUIET_STOP_TIMEOUT, RequestError, RunRequestError, Runners, state::RunnerState},
  tasks::changes::{ChangesError, NotCheckpointed, ReadChanges, RunStage, not_checkpointed_in, run_worktree},
  workspace::OpenProject,
};
use runner_programs::programs;
use tempfile::TempDir;
use tokio::{runtime::Handle, time::timeout};

const PATIENCE: Duration = Duration::from_secs(60);

fn git(dir: &Path, args: &[&str]) {
  let output = Command::new("git")
    .arg("-C")
    .arg(dir)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

/// A git repository on `main` with one commit, set up as the project Kontur, and the worktree of `run` on a branch of
/// its own, as the runner makes it.
fn project(run: RunId) -> (TempDir, OpenProject) {
  let folder = tempfile::tempdir().expect("temporary directory");
  let root = folder.path();
  git(root, &["init", "--quiet", "--initial-branch=main"]);
  git(root, &["config", "user.name", "Orchester Test"]);
  git(root, &["config", "user.email", "test@orchester.invalid"]);
  git(root, &["config", "commit.gpgsign", "false"]);
  fs::write(root.join(".gitignore"), ".orchester/\n").expect("the ignore file");
  fs::write(root.join("README.md"), "hi\n").expect("the readme");
  fs::create_dir(root.join("src")).expect("src");
  fs::write(root.join("src").join("lib.rs"), "pub fn one() -> u32 {\n  1\n}\n").expect("the library");
  git(root, &["add", "."]);
  git(root, &["commit", "--quiet", "-m", "initial"]);
  let worktree = run_worktree(root, run);
  let path = worktree.to_str().expect("a path git reads");
  git(root, &["worktree", "add", "--quiet", "-b", "kon-1", path]);
  let open = OpenProject {
    project: orchester_core::project::Project::new(
      orchester_core::id::ProjectId::generate(),
      orchester_core::project::ProjectKey::new("KON").expect("key"),
      orchester_core::name::Name::new("Kontur").expect("name"),
    ),
    repo_root: root.to_path_buf(),
  };
  drop(orchester_store::ProjectDb::create(root, open.project.clone()).expect("create project"));
  (folder, open)
}

/// What the page showed when the desktop read the worktree itself, with git.
fn read_by_git(worktree: &Path) -> NotCheckpointed {
  let read = read_worktree_changes(&ProcessRunner::new(Duration::from_secs(30)), Path::new("git"), worktree)
    .expect("git reads the worktree");
  let WorktreeRead::Read(text) = read.read else {
    panic!("changes to read: {:?}", read.read);
  };
  let mut hasher = DefaultHasher::new();
  text.hash(&mut hasher);
  NotCheckpointed::Read(Arc::new(ReadChanges {
    key: format!("{}:{:016x}", read.head, hasher.finish()).into(),
    diff: Arc::new(Diff::parse(&text).expect("a unified diff")),
    from: read.head,
  }))
}

#[tokio::test(flavor = "multi_thread")]
async fn worktree_changes_are_read_by_the_runner_it_starts_and_match_what_git_reads() {
  let run = RunId::generate();
  let (_folder, open) = project(run);
  let worktree = run_worktree(&open.repo_root, run);
  fs::write(worktree.join("README.md"), "hi\nhello\n").expect("change the readme");
  fs::write(worktree.join("src").join("lib.rs"), "pub fn one() -> u32 {\n  2\n}\n").expect("change the library");
  fs::write(worktree.join("notes.md"), "draft\n").expect("an untracked file");
  // No tab shows the project: its runner is not running until the read starts it.
  let runners = Runners::new(programs().runner.clone(), Handle::current(), Activity::new(), Clean);
  assert!(runners.status_of(&open.repo_root).is_none());

  let read = timeout(PATIENCE, not_checkpointed_in(&runners, &open, (run, RunStage::Started)))
    .await
    .expect("read in time")
    .expect("the runner reads the worktree");
  assert_eq!(
    read,
    read_by_git(&worktree),
    "the same files, diff and key as git's own read"
  );
  let NotCheckpointed::Read(changes) = &read else {
    panic!("changes: {read:?}");
  };
  let paths: Vec<&str> = changes.diff.files().iter().map(|file| file.path()).collect();
  assert_eq!(paths, ["README.md", "src/lib.rs", "notes.md"]);
  assert_eq!(
    runners.status_of(&open.repo_root).map(|status| status.state().clone()),
    Some(RunnerState::Online),
    "the read started the project's runner"
  );

  git(&worktree, &["checkout", "--quiet", "--", "."]);
  fs::remove_file(worktree.join("notes.md")).expect("remove the untracked file");
  let clean = timeout(PATIENCE, not_checkpointed_in(&runners, &open, (run, RunStage::Started)))
    .await
    .expect("read in time");
  assert!(matches!(clean, Ok(NotCheckpointed::Clean)), "{clean:?}");

  // A run without a worktree folder: not made yet, removed, or missing, by its stage.
  let other = RunId::generate();
  let read = |stage| not_checkpointed_in(&runners, &open, (other, stage));
  assert!(matches!(read(RunStage::NotStarted).await, Ok(NotCheckpointed::Clean)));
  assert!(matches!(read(RunStage::Ended).await, Ok(NotCheckpointed::Removed)));
  let missing = read(RunStage::Started)
    .await
    .expect_err("a started run's worktree is there");
  assert!(matches!(missing, ChangesError::WorktreeMissing(_)), "{missing}");
  assert_eq!(missing.message().key, "runs.changes.failure.worktreeMissing");

  runners.shutdown(Some(QUIET_STOP_TIMEOUT)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn worktree_changes_are_an_explicit_error_when_the_runner_cannot_start() {
  let run = RunId::generate();
  let (folder, open) = project(run);
  fs::write(run_worktree(&open.repo_root, run).join("notes.md"), "draft\n").expect("an untracked file");
  let missing = folder.path().join("no-runner").join("orchester-runner.exe");
  let runners = Runners::new(missing, Handle::current(), Activity::new(), Clean);

  let read = timeout(PATIENCE, not_checkpointed_in(&runners, &open, (run, RunStage::Started)))
    .await
    .expect("answer in time");
  let error = read.expect_err("no runner, no read: the folder is not read by the desktop instead");
  assert!(
    matches!(
      error,
      ChangesError::Runner(RunRequestError::Request(RequestError::NotConnected))
    ),
    "{error}"
  );
  assert_eq!(error.message().key, "runs.changes.failure.runner");
  runners.shutdown(Some(QUIET_STOP_TIMEOUT)).await;
}
