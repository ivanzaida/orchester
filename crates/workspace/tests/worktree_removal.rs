//! Removing a run's worktrees from the task page when the run changed since the page read it, and
//! an interrupted run whose worktree folder is gone (ORC-63); a removal ends only once the folder is gone, build
//! output included, a folder that stays is recorded as not removed with why, and folders no run uses are swept
//! (ORC-418).

mod support;

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command as Process,
  sync::{Arc, mpsc},
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  integration::{CleanupError, Leftovers},
  run::{InterruptCause, Run},
  store::WorktreeStore,
};
use orchester_exec::{ProcessRunner, programs::Programs};
use orchester_store::ProjectDb;
use orchester_workspace::{
  activity::Activity,
  tasks::{
    detail::TaskDetail,
    integration::{IntegrationProblem, leftovers_in, remove_in},
    integration_jobs::{IntegrationJobs, RemoveArgs, Remover},
    worktree_left::not_removed,
  },
  workspace::OpenProject,
};
use support::{Fixture, at};

#[test]
fn worktree_removal_is_refused_for_a_run_continued_since_the_page_read_it() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  let events = run.interrupt(InterruptCause::QuitNow, at(30)).expect("interrupt");
  fixture.record(&run, &events);
  // What the page read; another window then continues the run.
  let read = run.clone();
  let events = run.continue_run(at(40)).expect("continue");
  fixture.record(&run, &events);

  let refused = remove_in(
    fixture.repo.path(),
    &Programs::processes(),
    &read,
    &Leftovers::default(),
  );
  assert_eq!(refused, Err(IntegrationProblem::Cleanup(CleanupError::LiveWork)));
  let db = ProjectDb::open(fixture.repo.path()).expect("open");
  assert_eq!(
    db.worktree_removal(run.id()).expect("read"),
    None,
    "nothing is recorded"
  );
}

#[test]
fn worktree_removal_is_refused_for_a_run_that_worked_again_in_another_worktree_since_the_page_read_it() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  fixture.succeed(&mut run, at(30));
  // What the page read; another window then asks the run for a follow-up, which works in a new worktree
  // and is interrupted: the run has no live work, but more worktrees than the page listed.
  let read = run.clone();
  let step = run.snapshot().steps()[0].key().clone();
  let events = run.follow_up(&step, at(40)).expect("follow up");
  fixture.record(&run, &events);
  let events = run.interrupt(InterruptCause::QuitNow, at(50)).expect("interrupt");
  fixture.record(&run, &events);

  let refused = remove_in(
    fixture.repo.path(),
    &Programs::processes(),
    &read,
    &Leftovers::default(),
  );
  assert_eq!(refused, Err(IntegrationProblem::RunChanged));
  let db = ProjectDb::open(fixture.repo.path()).expect("open");
  assert_eq!(
    db.worktree_removal(run.id()).expect("read"),
    None,
    "nothing is recorded"
  );
}

#[test]
fn worktree_removal_is_recorded_once_an_interrupted_run_is_found_without_its_folder() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  let events = run.interrupt(InterruptCause::QuitNow, at(30)).expect("interrupt");
  fixture.record(&run, &events);

  // The fixture's run never had a worktree folder: the page records it as removed.
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("the task page reads");
  let latest = detail.run.as_ref().expect("the run");
  assert!(latest.worktree_removed.is_some());
  assert!(detail.history[0].removed_at.is_some());
}

#[test]
fn worktree_removal_automation_asked_for_tells_it_the_run_changed() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  fixture.succeed(&mut run, at(30));
  // What the tool read; the run then works again in a new worktree and is interrupted.
  let read = run.clone();
  let step = run.snapshot().steps()[0].key().clone();
  let events = run.follow_up(&step, at(40)).expect("follow up");
  fixture.record(&run, &events);
  let events = run.interrupt(InterruptCause::QuitNow, at(50)).expect("interrupt");
  fixture.record(&run, &events);

  let open = OpenProject {
    project: fixture.db.project().clone(),
    repo_root: fixture.repo.path().to_path_buf(),
  };
  let (answer, outcome) = mpsc::channel();
  IntegrationJobs::default().of(run.id()).start_remove(RemoveArgs {
    open,
    task,
    key: "ORC-1".into(),
    number: run.number().get(),
    run: read,
    confirmed: Leftovers::default(),
    programs: Programs::processes(),
    activity: Activity::new(),
    separator: ", ".into(),
    changed: Arc::new(|| {}),
    by: Remover::Automation(answer),
  });
  let refused = outcome.recv().expect("an answer").expect_err("refused");
  assert!(refused.run_changed, "{refused:?}");
  assert_eq!(refused.message.key, "runs.worktree.failure.runChanged");
}

fn git(dir: &Path, args: &[&str]) -> String {
  let output = Process::new("git")
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
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Makes the fixture's folder a repository with one commit on `main`: `.orchester/` excluded, and build output and
/// `.tmp/` ignored, as a project's `.gitignore` has them.
fn repository(root: &Path) {
  git(root, &["init", "--quiet", "--initial-branch=main"]);
  git(root, &["config", "user.name", "Orchester Test"]);
  git(root, &["config", "user.email", "test@orchester.invalid"]);
  git(root, &["config", "core.autocrlf", "false"]);
  git(root, &["config", "commit.gpgsign", "false"]);
  fs::write(root.join(".git/info/exclude"), ".orchester/\n").expect("exclude .orchester");
  fs::write(root.join(".gitignore"), "target/\n.tmp/\n").expect("ignore build output");
  git(root, &["add", ".gitignore"]);
  git(root, &["commit", "--quiet", "-m", "initial"]);
}

/// The worktree of `run` as the runner adds it, `.orchester/worktrees/<run>` on the run's branch.
fn add_worktree(root: &Path, run: &Run) -> PathBuf {
  let folder = root.join(".orchester/worktrees").join(run.id().to_string());
  let path = folder.to_str().expect("a UTF-8 path");
  git(
    root,
    &["worktree", "add", "--quiet", "-b", run.branch().as_str(), path, "main"],
  );
  folder
}

/// What a build and a QA copy leave in `folder`, all of it ignored by git; the file a running build would hold.
fn build_output(folder: &Path) -> PathBuf {
  fs::create_dir_all(folder.join("target/debug")).expect("build folder");
  fs::create_dir_all(folder.join(".tmp/qa")).expect("scratch folder");
  fs::write(folder.join(".tmp/qa/log.txt"), "qa\n").expect("scratch file");
  let built = folder.join("target/debug/app.bin");
  fs::write(&built, "built\n").expect("build output");
  built
}

/// What `run` left in the repository at `root`, as the task page reads it before a removal.
fn left(root: &Path, programs: &Programs, run: &Run) -> Leftovers {
  leftovers_in(root, programs, &[Arc::new(run.clone())])
    .expect("what the run left")
    .0
    .remove(0)
    .1
}

/// Git as on Windows when it stops part way: `git worktree remove` forgets the worktree, then stops at its build
/// output with "Directory not empty", which stays on disk.
struct StopsAtBuildOutput(ProcessRunner);

impl CommandRunner for StopsAtBuildOutput {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let args = command.args();
    if !(args.len() > 1 && args[0] == "worktree" && args[1] == "remove") {
      return self.0.run(command);
    }
    let removed = self.0.run(command)?;
    assert!(removed.success(), "git removes the worktree: {}", removed.stderr);
    let folder = PathBuf::from(args.last().expect("the worktree's path"));
    build_output(&folder);
    Ok(CommandOutput {
      code: Some(255),
      stdout: String::new(),
      stderr: format!("error: failed to delete '{}': Directory not empty", folder.display()),
    })
  }
}

#[test]
fn worktree_removal_deletes_the_build_output_git_leaves_behind() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  fixture.succeed(&mut run, at(30));
  let root = fixture.repo.path().to_path_buf();
  repository(&root);
  let folder = add_worktree(&root, &run);
  build_output(&folder);
  let programs = Programs::with_runner(Arc::new(StopsAtBuildOutput(ProcessRunner::new(Duration::from_secs(
    60,
  )))));
  let seen = left(&root, &programs, &run);
  assert!(!seen.loses_files(), "ignored build output is not work: {seen:?}");

  remove_in(&root, &programs, &run, &seen).expect("removed");

  assert!(!folder.exists(), "the folder is gone, build output and .tmp included");
  assert!(
    !git(&root, &["worktree", "list", "--porcelain"]).contains(&run.id().to_string()),
    "git no longer lists it"
  );
  let db = ProjectDb::open(&root).expect("open");
  assert!(db.worktree_removal(run.id()).expect("read").is_some());
  assert_eq!(not_removed(&db, &root, run.id()).expect("read"), None);
  let detail = TaskDetail::load_in(&root, task).expect("the task page reads");
  assert_eq!(detail.run.expect("the run").worktree_not_removed, None);
}

/// Keeps a file of a worktree from being deleted while it lives, as a running build or QA copy does: open without
/// sharing on Windows; elsewhere its folder cannot be written.
struct Hold {
  #[cfg(windows)]
  _file: fs::File,
  #[cfg(not(windows))]
  folder: PathBuf,
}

impl Hold {
  fn new(file: &Path) -> Self {
    #[cfg(windows)]
    {
      use std::os::windows::fs::OpenOptionsExt;
      let held = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(file)
        .expect("hold the file");
      Self { _file: held }
    }
    #[cfg(not(windows))]
    {
      use std::os::unix::fs::PermissionsExt;
      let folder = file.parent().expect("the file's folder").to_path_buf();
      fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).expect("keep the folder");
      Self { folder }
    }
  }
}

#[cfg(not(windows))]
impl Drop for Hold {
  fn drop(&mut self) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&self.folder, fs::Permissions::from_mode(0o755)).expect("free the folder");
  }
}

#[test]
fn worktree_removal_of_a_held_folder_is_recorded_as_not_removed_with_why() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  fixture.succeed(&mut run, at(30));
  let root = fixture.repo.path().to_path_buf();
  repository(&root);
  let folder = add_worktree(&root, &run);
  let hold = Hold::new(&build_output(&folder));
  let programs = Programs::processes();
  let seen = left(&root, &programs, &run);

  let refused = remove_in(&root, &programs, &run, &seen);

  let Err(IntegrationProblem::NotRemoved { path, detail }) = refused else {
    panic!("the held folder stays: {refused:?}");
  };
  assert!(folder.is_dir());
  let db = ProjectDb::open(&root).expect("open");
  let recorded = not_removed(&db, &root, run.id())
    .expect("read")
    .expect("recorded as not removed");
  assert_eq!(recorded.path, path);
  assert_eq!(recorded.reason.as_deref(), Some(detail.as_str()));
  let detail = TaskDetail::load_in(&root, task).expect("the task page reads");
  assert_eq!(
    detail.run.expect("the run").worktree_not_removed,
    Some(recorded),
    "the task page says not removed, never removed"
  );

  // Once nothing holds it, Remove worktree finishes the removal.
  drop(hold);
  let seen = left(&root, &programs, &run);
  remove_in(&root, &programs, &run, &seen).expect("removed");
  assert!(!folder.exists());
  assert_eq!(not_removed(&db, &root, run.id()).expect("read"), None);
}

#[test]
fn worktree_removal_sweeps_folders_no_run_uses_and_never_a_running_runs() {
  let mut fixture = Fixture::new();
  let ended = |fixture: &mut Fixture, title: &str| {
    let task = fixture.task(title, at(10));
    let mut run = fixture.started_run(task, at(20));
    fixture.succeed(&mut run, at(30));
    run
  };
  let orphaned = ended(&mut fixture, "Merged long ago");
  let listed = ended(&mut fixture, "Waits for review");
  let removed = ended(&mut fixture, "Merged now");
  let running_task = fixture.task("At work", at(10));
  let running = fixture.started_run(running_task, at(20));
  let root = fixture.repo.path().to_path_buf();
  repository(&root);
  let worktrees = root.join(".orchester/worktrees");
  // What a removal that failed part way left: git forgot it, its build output stayed.
  let orphan = worktrees.join(orphaned.id().to_string());
  build_output(&orphan);
  let at_work = worktrees.join(running.id().to_string());
  build_output(&at_work);
  let unknown = worktrees.join("notes");
  build_output(&unknown);
  let kept = add_worktree(&root, &listed);
  let folder = add_worktree(&root, &removed);
  let programs = Programs::processes();

  let seen = left(&root, &programs, &removed);
  remove_in(&root, &programs, &removed, &seen).expect("removed");

  assert!(!folder.exists(), "the removed worktree is gone");
  // The sweep runs on a thread of its own: the removal does not wait for it.
  let deadline = Instant::now() + Duration::from_secs(60);
  while orphan.exists() {
    assert!(Instant::now() < deadline, "an ended run's folder git forgot is swept");
    thread::sleep(Duration::from_millis(20));
  }
  assert!(at_work.is_dir(), "a running run's folder is never touched");
  assert!(unknown.is_dir(), "a folder no run has is left alone");
  assert!(kept.is_dir(), "a worktree git lists stays");
}
