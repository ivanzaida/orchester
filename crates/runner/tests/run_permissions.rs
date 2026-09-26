//! Permissions changed for a run (02.J) end to end against `orchester-fake-codex`: a change applies
//! from the next invocation; Apply now lets the agent finish its command, commits its work as a
//! checkpoint and restarts it, resuming its session, with the new permissions; keeping the change
//! for the next invocation cancels the restart; beside other steps every agent of the profile
//! restarts. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test run_permissions run_permissions`.

mod support;

use std::path::Path;

use orchester_core::{
  connection::stream::AgentEvent,
  id::{IdempotencyKey, ResourceId, RunId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::StepKey,
  run::{Checkpoint, PermissionsApply, Run, RunEventKind, RunStatus},
  store::{ActivityStore, RunStore},
};
use orchester_ipc::{Event, Rejection};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, StepSpec, git, project, reported, scheduler, wait_for};

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("key")
}

/// Read-only: Codex runs it in its `read-only` sandbox, where the profile's steps write.
fn read_only() -> AgentPermissions {
  AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  )
}

fn builder(root: &Path, run: RunId) -> ResourceId {
  let run = ProjectDb::open(root)
    .expect("open")
    .run(run)
    .expect("read")
    .expect("run");
  run
    .profile_of(&run.snapshot().steps()[0].key().clone())
    .expect("an agent step")
}

/// Waits until `count` commands of the run have started.
async fn wait_for_commands(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId, count: usize) {
  let db = ProjectDb::open(root).expect("open");
  let started = async {
    while db
      .activity(run, 0)
      .expect("activity")
      .iter()
      .filter(|entry| matches!(&entry.event, AgentEvent::CommandStarted { command, .. } if command == "pnpm test"))
      .count()
      < count
    {
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, started)
    .await
    .expect("the command starts in time");
}

/// Lets the fake CLI's `pnpm test` finish.
fn release(root: &Path) {
  std::fs::write(root.join(".orchester").join("fake-release"), "go").expect("release");
}

async fn start(setup: &Setup) -> (support::TestProject, Scheduler, mpsc::UnboundedReceiver<Event>, RunId) {
  let project = project(setup);
  let (scheduler, events) = scheduler(project.repo.path());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  (project, scheduler, events, id)
}

fn finished(run: &Run) -> bool {
  run.status().is_terminal()
}

#[tokio::test(flavor = "multi_thread")]
async fn run_permissions_apply_from_the_next_invocation() {
  let (project, scheduler, mut events, id) = start(&Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Wait for release.", &[]),
      StepSpec::new("review", "Review", "Say hello.", &["implement"]),
    ],
    ..Setup::new("Say hello.")
  })
  .await;
  let root = project.repo.path();
  wait_for_commands(root, &mut events, id, 1).await;
  let builder = builder(root, id);
  scheduler
    .change_permissions(id, builder, read_only(), PermissionsApply::NextInvocation)
    .await
    .expect("the change is recorded");
  release(root);
  let run = wait_for(root, &mut events, id, finished).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.invocations(id).expect("invocations").len(), 2, "nothing restarted");
  let argv = reported(&db, id, "argv: ");
  assert!(argv[0].contains("--sandbox workspace-write"), "{argv:?}");
  assert!(argv[1].contains("--sandbox read-only"), "the next invocation: {argv:?}");
  let changed = run.changed_permissions(builder).expect("changed");
  assert_eq!(changed.permissions(), &read_only());
  assert!(changed.applied().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn run_permissions_apply_now_restarts_after_the_command_with_the_same_session() {
  let (project, scheduler, mut events, id) = start(&Setup {
    steps: vec![StepSpec::new(
      "implement",
      "Implement",
      "Wait for release. Draft first. Then keep working.",
      &[],
    )],
    ..Setup::new("Say hello.")
  })
  .await;
  let root = project.repo.path();
  wait_for_commands(root, &mut events, id, 1).await;
  let builder = builder(root, id);
  scheduler
    .change_permissions(id, builder, read_only(), PermissionsApply::Now)
    .await
    .expect("the change is recorded");
  let db = ProjectDb::open(root).expect("open");
  let pending = db.run(id).expect("read").expect("run");
  assert!(pending.changed_permissions(builder).expect("changed").is_pending());
  // The command it runs finishes first: the agent is still at work.
  assert_eq!(db.invocations(id).expect("invocations")[0].ended_at, None);
  release(root);
  let run = wait_for(root, &mut events, id, finished).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 2);
  assert_eq!(invocations[0].execution, invocations[1].execution);
  assert_eq!(invocations[1].session.as_deref(), Some("fake-thread-1"));
  let argv = reported(&db, id, "argv: ");
  assert!(argv[0].contains("--sandbox workspace-write"), "{argv:?}");
  assert!(
    argv[1].contains("exec resume") && argv[1].contains("sandbox_mode=read-only"),
    "the session resumes with the new permissions: {argv:?}"
  );
  let applied = run
    .changed_permissions(builder)
    .and_then(|changed| changed.applied())
    .expect("applied");
  let checkpoint = applied.checkpoint.expect("the work was saved");
  let events_log = db.events(id).expect("events");
  let saved = events_log
    .iter()
    .find_map(|event| match event.kind() {
      RunEventKind::CheckpointRecorded {
        checkpoint: recorded,
        revision,
        ..
      } if *recorded == checkpoint => Some(revision.clone()),
      _ => None,
    })
    .expect("the checkpoint is recorded");
  assert_eq!(
    git(root, &["show", &format!("{}:README.md", saved.as_str())]),
    "hi\ndraft"
  );
  assert!(git(root, &["log", "--format=%s", "-1", saved.as_str()]).contains("saved to apply permissions"));
  assert!(run.last_checkpoint().map(Checkpoint::id).is_some());
  assert_eq!(
    git(root, &["show", &format!("{}:README.md", run.branch())]),
    "hi\ndraft\ncontinued"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn run_permissions_kept_for_next_invocation_do_not_restart() {
  let (project, scheduler, mut events, id) = start(&Setup {
    steps: vec![StepSpec::new("implement", "Implement", "Wait for release.", &[])],
    ..Setup::new("Say hello.")
  })
  .await;
  let root = project.repo.path();
  wait_for_commands(root, &mut events, id, 1).await;
  let builder = builder(root, id);
  assert_eq!(
    scheduler.keep_permissions_for_next_invocation(id, builder).await,
    Err(Rejection::NoPendingPermissions)
  );
  scheduler
    .change_permissions(id, builder, read_only(), PermissionsApply::Now)
    .await
    .expect("the change is recorded");
  scheduler
    .keep_permissions_for_next_invocation(id, builder)
    .await
    .expect("the restart is cancelled");
  release(root);
  let run = wait_for(root, &mut events, id, finished).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    db.invocations(id).expect("invocations").len(),
    1,
    "the agent finished its invocation"
  );
  let changed = run.changed_permissions(builder).expect("changed");
  assert!(!changed.is_pending());
  assert!(changed.applied().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn run_permissions_apply_now_restarts_every_agent_beside_others() {
  let (project, scheduler, mut events, id) = start(&Setup {
    steps: vec![
      StepSpec::new("checks", "Checks", "Wait for release. Then keep working.", &[]),
      StepSpec::new("review", "Review", "Wait for release. Then keep working.", &[]),
    ],
    ..Setup::new("Say hello.")
  })
  .await;
  let root = project.repo.path();
  wait_for_commands(root, &mut events, id, 2).await;
  let builder = builder(root, id);
  assert_eq!(
    scheduler
      .change_permissions(id, ResourceId::generate(), read_only(), PermissionsApply::Now)
      .await,
    Err(Rejection::ProfileNotInRun)
  );
  scheduler
    .change_permissions(id, builder, read_only(), PermissionsApply::Now)
    .await
    .expect("the change is recorded");
  release(root);
  let run = wait_for(root, &mut events, id, finished).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 4, "both agents restarted");
  for step in ["checks", "review"] {
    let execution = run.latest_execution(&key(step)).expect("execution").id();
    let restarted = db
      .events(id)
      .expect("events")
      .iter()
      .any(|event| matches!(event.kind(), RunEventKind::PermissionsApplied { execution: applied, .. } if *applied == execution));
    assert!(restarted, "{step} restarted with the new permissions");
  }
  let resumed = reported(&db, id, "argv: ")
    .into_iter()
    .filter(|argv| argv.contains("exec resume") && argv.contains("sandbox_mode=read-only"))
    .count();
  assert_eq!(resumed, 2);
}
