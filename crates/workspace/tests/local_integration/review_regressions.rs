use std::{
  fs,
  io::Write,
  process::{Command, Stdio},
};

use orchester_core::{
  id::{IdempotencyKey, RunId},
  integration::local::{LocalError, LocalIntegrationStore, LocalPhase, PendingResolution, Resolution, ResolutionWrite},
  name::Name,
  revision::Revision,
  run::{BranchName, NewRun},
  store::{RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_exec::local_git::{LocalGit, digest};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::local_integration::{IntegrationError, LocalIntegration};

use super::support::{Fixture, git};

fn task_git(fixture: &Fixture) -> LocalGit {
  LocalGit::new(
    &fixture.task_root,
    &fixture.task_root.join(".orchester/local-integration"),
  )
  .expect("task Git")
}

fn moved_base(fixture: &Fixture, conflicting: bool) {
  let path = if conflicting { "task.txt" } else { "main-new.txt" };
  fs::write(fixture.root().join(path), b"new main work\n").expect("main edit");
  git(fixture.root(), &["add", path]);
  git(fixture.root(), &["commit", "-m", "new main commit"]);
}

fn save_operation(
  fixture: &Fixture,
  session: &LocalIntegration,
  mut operation: orchester_core::integration::local::LocalOperation,
) {
  operation.advance_version();
  assert!(
    ProjectDb::open(fixture.root())
      .expect("db")
      .save_local_operation(&operation, Some(session.operation().version()))
      .expect("save intent")
  );
}

fn interrupted_sync(fixture: &Fixture) {
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  let task = task_git(fixture);
  let before = task.snapshot().expect("before");
  let result = task
    .preview_sync(&before, &session.operation().reviewed().head)
    .expect("preview");
  let mut operation = session.operation().clone();
  operation
    .begin_sync(session.operation().reviewed())
    .expect("sync intent");
  operation.own_task(before);
  operation.plan_sync_result(result).expect("planned identity");
  let base = operation.reviewed().head.clone();
  save_operation(fixture, &session, operation);
  let _outcome = task.text(&[
    "-c",
    "merge.conflictStyle=merge",
    "merge",
    "--no-ff",
    "--no-commit",
    "--no-overwrite-ignore",
    &base,
  ]);
  assert!(task.snapshot().expect("actual merge").operation.is_some());
}

#[test]
fn local_integration_interrupted_sync_conflicts_resume_and_check_without_adopting_foreign_work() {
  let fixture = Fixture::new(false);
  moved_base(&fixture, true);
  interrupted_sync(&fixture);
  let mut session = fixture.open();
  session.reconcile().expect("recover owned pending merge");
  assert_eq!(session.operation().phase(), LocalPhase::TaskConflicts);
  let task = task_git(&fixture);
  let file = task.conflicts().expect("conflicts").remove(0);
  let before = task.file(&file.path).expect("file").as_deref().map(digest);
  session
    .save_task_resolution(
      &file,
      before.as_ref(),
      &Resolution::Text("combined reviewed result\n".into()),
    )
    .expect("resolve");
  session.continue_task().expect("fresh Check");
  assert_eq!(session.operation().phase(), LocalPhase::AwaitingApproval);
  assert_eq!(
    git(&fixture.task_root, &["show", "HEAD:task.txt"]),
    "combined reviewed result"
  );
}

#[test]
fn local_integration_interrupted_sync_can_abort_only_exact_owned_state() {
  let fixture = Fixture::new(false);
  moved_base(&fixture, true);
  let before = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  interrupted_sync(&fixture);
  let mut session = fixture.open();
  session.reconcile().expect("recover");
  session.abort_task().expect("scoped abort");
  assert_eq!(git(&fixture.task_root, &["rev-parse", "HEAD"]), before);
  assert_eq!(git(&fixture.task_root, &["status", "--porcelain"]), "");
}

#[test]
fn local_integration_interrupted_sync_refuses_external_work_and_keeps_it() {
  let fixture = Fixture::new(false);
  moved_base(&fixture, true);
  interrupted_sync(&fixture);
  fs::write(fixture.task_root.join("foreign.txt"), b"foreign retained\n").expect("foreign edit");
  let mut session = fixture.open();
  assert!(session.reconcile().is_err());
  assert!(session.abort_task().is_err());
  assert_eq!(
    fs::read(fixture.task_root.join("foreign.txt")).expect("retained"),
    b"foreign retained\n"
  );
  assert!(task_git(&fixture).operation().expect("marker").is_some());
}

fn hook(fixture: &Fixture, fails: bool) {
  let path = fixture.root().join(".git/hooks/pre-commit");
  fs::write(
    &path,
    if fails {
      "#!/bin/sh\nexit 1\n"
    } else {
      "#!/bin/sh\nexit 0\n"
    },
  )
  .expect("private hook");
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("hook mode");
  }
}

#[test]
fn local_integration_sync_hook_failure_retains_owned_merge_for_restart_and_retry() {
  let fixture = Fixture::new(false);
  moved_base(&fixture, false);
  hook(&fixture, true);
  let before = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  assert!(session.sync().is_err());
  assert_eq!(session.operation().phase(), LocalPhase::TaskConflicts);
  assert!(session.operation().checked().is_none());
  assert_eq!(git(&fixture.task_root, &["rev-parse", "HEAD"]), before);
  let pending = task_git(&fixture).snapshot().expect("owned merge");
  assert_eq!(session.operation().owned_task(), Some(&pending));
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("restart");
  hook(&fixture, false);
  session.continue_task().expect("retry commit and fresh Check");
  assert_eq!(session.operation().phase(), LocalPhase::AwaitingApproval);
  assert_ne!(git(&fixture.task_root, &["rev-parse", "HEAD"]), before);
}

#[test]
fn local_integration_sync_hook_failure_allows_scoped_abort_after_restart() {
  let fixture = Fixture::new(false);
  moved_base(&fixture, false);
  hook(&fixture, true);
  let before = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  assert!(session.sync().is_err());
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("restart");
  session.abort_task().expect("abort clean pending merge");
  assert_eq!(git(&fixture.task_root, &["rev-parse", "HEAD"]), before);
  assert_eq!(git(&fixture.task_root, &["status", "--porcelain"]), "");
}

#[test]
fn local_integration_restart_records_completed_sync_commit_without_repeating_it() {
  let fixture = Fixture::new(false);
  moved_base(&fixture, false);
  hook(&fixture, true);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  assert!(session.sync().is_err());
  let mut operation = session.operation().clone();
  operation.plan_task_commit().expect("commit intent");
  let action = operation.task_commit_action().expect("receipt action").to_owned();
  save_operation(&fixture, &session, operation);
  hook(&fixture, false);
  task_git(&fixture)
    .commit_task_resolution("Resolve task integration conflicts", &action)
    .expect("actual commit");
  let merged = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("receipt reconciliation and Check");
  assert_eq!(session.operation().phase(), LocalPhase::AwaitingApproval);
  assert_eq!(git(&fixture.task_root, &["rev-parse", "HEAD"]), merged);
}

fn second_run(fixture: &Fixture) -> RunId {
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  let first = db.run(fixture.run).expect("run").expect("exists");
  let now = Timestamp::now();
  let task = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![TaskDetails {
        title: Name::new("Second task").expect("name"),
        brief: String::new(),
        acceptance_criteria: Vec::new(),
        pipeline: None,
        limits: TaskLimits::NONE,
      }],
      now,
    )
    .expect("task")
    .remove(0);
  let mut run = db
    .create_run(
      IdempotencyKey::generate(),
      NewRun {
        task: task.id(),
        snapshot: first.snapshot().clone(),
        branch: BranchName::new("second").expect("branch"),
        base: Revision::new(&git(fixture.root(), &["rev-parse", "HEAD"])).expect("base"),
        base_branch: Some(BranchName::new("main").expect("branch")),
        budget: None,
        max_duration: None,
      },
      now,
    )
    .expect("run");
  let events = run.start(now).expect("start");
  db.append_events(&run, &events).expect("events");
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.start_step(execution, now).expect("step");
  db.append_events(&run, &events).expect("events");
  let events = run.complete_step(execution, now).expect("complete");
  db.append_events(&run, &events).expect("events");
  let path = fixture.root().join(".orchester/worktrees").join(run.id().to_string());
  git(
    fixture.root(),
    &["worktree", "add", "-b", "second", path.to_str().expect("path")],
  );
  fs::write(path.join("second.txt"), b"second task\n").expect("second edit");
  git(&path, &["add", "second.txt"]);
  git(&path, &["commit", "-m", "second result"]);
  run.id()
}

#[test]
fn local_integration_another_run_cannot_advance_checkout_before_restoration_and_cleanup() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), b"retained local edits\n").expect("edit");
  let mut session = fixture.open();
  session.preserve("First task recovery").expect("preserve");
  session.sync().expect("Check");
  let checked = session.operation().checked().expect("checked").clone();
  session.approve_and_merge(&checked).expect("merge");
  let first_head = git(fixture.root(), &["rev-parse", "HEAD"]);
  drop(session);
  let second = second_run(&fixture);
  assert!(matches!(
    LocalIntegration::open(fixture.root(), second),
    Err(IntegrationError::State(LocalError::ForeignOperation))
  ));
  assert!(
    ProjectDb::open(fixture.root())
      .expect("db")
      .local_operation(second)
      .expect("no journal")
      .is_none()
  );
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), first_head);
  let mut session = fixture.open();
  session.reconcile().expect("first recovery remains reachable");
  session.restore().expect("restore first edits");
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("restored"),
    b"retained local edits\n"
  );
  drop(session);
  assert!(LocalIntegration::open(fixture.root(), second).is_err());
  let mut session = fixture.open();
  session.remove_preservation().expect("verified cleanup");
  drop(session);
  assert!(LocalIntegration::open(fixture.root(), second).is_ok());
}

#[test]
fn local_integration_resolution_replay_refuses_foreign_stages_of_another_unresolved_file() {
  let fixture = Fixture::new(false);
  fs::write(fixture.task_root.join("local.txt"), b"task local\n").expect("task edit");
  git(&fixture.task_root, &["commit", "-am", "another task edit"]);
  moved_base(&fixture, true);
  fs::write(fixture.root().join("local.txt"), b"main local\n").expect("main edit");
  git(fixture.root(), &["commit", "-am", "another main edit"]);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("conflicts");
  let task = task_git(&fixture);
  let conflicts = task.conflicts().expect("two conflicts");
  assert_eq!(conflicts.len(), 2);
  let file = conflicts
    .iter()
    .find(|file| file.path.as_str() == "task.txt")
    .expect("target");
  let other = conflicts
    .iter()
    .find(|file| file.path.as_str() == "local.txt")
    .expect("untouched");
  let before = task.snapshot().expect("before");
  let hash = task.file(&file.path).expect("file").as_deref().map(digest);
  let mut operation = session.operation().clone();
  operation
    .plan_resolution(PendingResolution {
      before,
      conflict: file.clone(),
      untouched_index: task
        .index_fingerprint_excluding(std::slice::from_ref(&file.path))
        .expect("full stage identity"),
      writes: vec![ResolutionWrite {
        path: file.path.clone(),
        before: hash,
        working: None,
        entry: None,
      }],
    })
    .expect("delete intent");
  save_operation(&fixture, &session, operation);
  let entry = other.current.as_ref().expect("stage 2");
  let input = format!("{:06o} {} 3\t{}\n", entry.mode, entry.object, other.path);
  let mut child = Command::new("git")
    .args(["update-index", "--index-info"])
    .current_dir(&fixture.task_root)
    .stdin(Stdio::piped())
    .spawn()
    .expect("foreign index writer");
  child
    .stdin
    .take()
    .expect("stdin")
    .write_all(input.as_bytes())
    .expect("write");
  assert!(child.wait().expect("exit").success());
  let foreign_index = git(&fixture.task_root, &["ls-files", "--stage"]);
  let working = fs::read(fixture.task_root.join("task.txt")).expect("original");
  drop(session);
  let mut session = fixture.open();
  assert!(matches!(
    session.reconcile(),
    Err(IntegrationError::State(LocalError::Stale))
  ));
  assert!(session.abort_task().is_err());
  assert_eq!(git(&fixture.task_root, &["ls-files", "--stage"]), foreign_index);
  assert_eq!(fs::read(fixture.task_root.join("task.txt")).expect("retained"), working);
}
