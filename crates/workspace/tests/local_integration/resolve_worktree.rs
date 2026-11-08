//! *Resolve in editor* on a run's worktree (`tasks::resolve`): the merge is recorded as the run's resolution before
//! git begins it, so the run neither continues nor is merged while it waits; *Abort* and *Continue* end it, and a
//! merge a person ended themselves ends it when the worktree is read again. Git in the worktree runs as the runner runs
//! it, here in the test's process through the protocol's messages.

use std::{
  fs,
  path::Path,
  sync::{Arc, mpsc},
  time::Duration,
};

use orchester_core::{
  id::ProjectId,
  integration::resolve::Started,
  name::Name,
  project::{Project, ProjectKey},
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::{
  tasks::{
    integration_jobs::{EditorOpened, JobState, ProjectJobs, ResolveArgs, ResolveJob, ResolveStep},
    resolve::{self, RunWorktrees},
  },
  workspace::OpenProject,
};

use super::support::{Fixture, git};

/// A succeeded run whose `task.txt` main changed too since it started.
fn conflicting() -> Fixture {
  let fixture = Fixture::recorded(false, "resolve-", "task", Some("delivered task\n"));
  fs::write(fixture.root().join("task.txt"), "main's task\n").expect("write");
  git(fixture.root(), &["commit", "-am", "main's change"]);
  fixture
}

fn programs() -> Programs {
  Programs::processes()
}

/// The runs' worktrees of the project at `root`.
fn worktrees(root: &Path) -> RunWorktrees {
  RunWorktrees::here_for_test(root, programs())
}

fn resolution(root: &Path, fixture: &Fixture) -> Option<orchester_core::time::Timestamp> {
  ProjectDb::open(root)
    .expect("db")
    .resolution(fixture.run)
    .expect("read")
}

#[test]
fn local_integration_resolve_records_the_merge_and_holds_the_run_back_until_it_ends() {
  let fixture = conflicting();
  let root = fixture.root();
  let started = resolve::start(&worktrees(root), root, fixture.run).expect("merged into the worktree");
  assert_eq!(started, Started::Conflicts(vec!["task.txt".to_owned()]));
  assert!(resolution(root, &fixture).is_some());
  let worktree = resolve::read(&worktrees(root), root, (fixture.run, true)).expect("read");
  assert!(worktree.present);
  assert!(worktree.merge.is_some());

  // The store refuses the run's continuations meanwhile (`orchester-store` `returned_after_end`).
  resolve::abort(&worktrees(root), root, fixture.run).expect("aborted");
  assert_eq!(resolution(root, &fixture), None);
  assert_eq!(git(&fixture.task_root, &["status", "--porcelain"]), "");
}

#[test]
fn local_integration_resolve_a_merge_ended_outside_orchester_ends_its_resolution() {
  let fixture = conflicting();
  let root = fixture.root();
  resolve::start(&worktrees(root), root, fixture.run).expect("merged into the worktree");
  git(&fixture.task_root, &["merge", "--abort"]);
  // While a resolving step works, a missing merge may be one it is about to begin: nothing ends then.
  resolve::read(&worktrees(root), root, (fixture.run, false)).expect("read");
  assert!(resolution(root, &fixture).is_some());
  let worktree = resolve::read(&worktrees(root), root, (fixture.run, true)).expect("read");
  assert!(worktree.merge.is_none());
  assert_eq!(resolution(root, &fixture), None);
}

#[test]
fn local_integration_resolve_a_held_run_is_not_merged_and_a_gone_worktree_ends_the_hold() {
  let fixture = conflicting();
  let root = fixture.root();
  resolve::start(&worktrees(root), root, fixture.run).expect("merged into the worktree");
  let main = git(root, &["rev-parse", "main"]);
  assert!(
    orchester_workspace::tasks::integration::merge_in(&fixture.worktrees(), &programs(), (fixture.run, None)).is_err(),
    "the merge waits in the worktree"
  );
  assert_eq!(git(root, &["rev-parse", "main"]), main, "git changed nothing");
  git(
    root,
    &[
      "worktree",
      "remove",
      "--force",
      fixture.task_root.to_str().expect("a path"),
    ],
  );
  let worktree = resolve::read(&worktrees(root), root, (fixture.run, false)).expect("read");
  assert!(!worktree.present);
  assert_eq!(resolution(root, &fixture), None);
}

#[test]
fn local_integration_resolve_continue_commits_and_ends_the_resolution() {
  let fixture = conflicting();
  let root = fixture.root();
  resolve::start(&worktrees(root), root, fixture.run).expect("merged into the worktree");
  assert!(
    resolve::finish(&worktrees(root), root, fixture.run).is_err(),
    "markers are left"
  );
  fs::write(fixture.task_root.join("task.txt"), "main's task\ndelivered task\n").expect("resolved");
  resolve::finish(&worktrees(root), root, fixture.run).expect("committed");
  assert_eq!(resolution(root, &fixture), None);
  let main = git(root, &["rev-parse", "main"]);
  git(&fixture.task_root, &["merge-base", "--is-ancestor", &main, "HEAD"]);
}

/// The job *Resolve in editor* starts on the run of `fixture`, ended: how it left the run's resolve job.
fn resolve_started(fixture: &Fixture) -> ResolveJob {
  let root = fixture.root();
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("TEST").expect("project key"),
    Name::new("Test project").expect("name"),
  );
  let open = OpenProject {
    project,
    repo_root: root.to_path_buf(),
  };
  let jobs = ProjectJobs::default().of(open.project.id()).of(fixture.run);
  let (ended, waiting) = mpsc::channel();
  let args = ResolveArgs {
    open,
    run: fixture.run,
    worktrees: worktrees(root),
    editor: Err(EditorOpened::NoEditor),
    changed: Arc::new(move || ended.send(()).expect("the test waits for the job")),
  };
  assert!(jobs.start_resolving(ResolveStep::Start, args), "the job began");
  waiting
    .recv_timeout(Duration::from_secs(60))
    .expect("the job ended in time");
  jobs.state().resolve
}

#[test]
fn local_integration_resolve_in_editor_on_a_branch_that_holds_the_base_says_nothing_conflicts() {
  // Main did not move since the run started: its head is in the run's branch, as after an agent merged it in.
  let fixture = Fixture::recorded(false, "resolve-", "task", Some("delivered task\n"));
  let before = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  let job = resolve_started(&fixture);
  assert_eq!(job.state, JobState::Succeeded);
  assert!(job.nothing_to_resolve, "the person is told there is nothing to resolve");
  assert_eq!(job.editor, None, "no editor opens on a merge that is not there");
  assert_eq!(
    git(&fixture.task_root, &["rev-parse", "HEAD"]),
    before,
    "git changed nothing"
  );
  assert_eq!(resolution(fixture.root(), &fixture), None);
}

#[test]
fn local_integration_resolve_in_editor_on_a_base_that_merges_cleanly_says_nothing_conflicts() {
  let fixture = Fixture::recorded(false, "resolve-", "task", Some("delivered task\n"));
  fs::write(fixture.root().join("other.txt"), "main's other file\n").expect("write");
  git(fixture.root(), &["add", "other.txt"]);
  git(fixture.root(), &["commit", "-m", "main's other change"]);
  let before = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  let job = resolve_started(&fixture);
  assert_eq!(job.state, JobState::Succeeded);
  assert!(job.nothing_to_resolve);
  assert_eq!(job.editor, None);
  assert_ne!(
    git(&fixture.task_root, &["rev-parse", "HEAD"]),
    before,
    "git merged main in"
  );
  assert_eq!(resolution(fixture.root(), &fixture), None);
}
