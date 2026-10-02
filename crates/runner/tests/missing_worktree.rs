//! A run cannot continue after someone deletes its worktree outside Orchester. Its stored state
//! and event history stay at the last settled point. Needs `git` and the fake Codex CLI.

mod support;

use std::{fs, path::Path};

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  run::{InterruptCause, PauseReason, Run, RunEvent, RunStatus},
  store::RunStore,
};
use orchester_ipc::Rejection;
use orchester_runner::{execution::Scheduler, worktree::Worktrees};
use orchester_store::ProjectDb;

use self::support::{PATIENCE, Setup, project, scheduler, wait_for, wait_for_idle, wait_for_turns};

async fn start(scheduler: &Scheduler, task: orchester_core::id::TaskId) -> RunId {
  scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the run starts")
}

fn stored(root: &Path, id: RunId) -> (Run, Vec<RunEvent>) {
  let db = ProjectDb::open(root).expect("open");
  (
    db.run(id).expect("read run").expect("run exists"),
    db.events(id).expect("read run events"),
  )
}

fn delete_worktree(root: &Path, id: RunId) {
  let path = Worktrees::new(root).expect("worktrees").path(id);
  assert!(path.starts_with(root.join(".orchester").join("worktrees")));
  assert!(path.is_dir(), "the run had a worktree before deletion");
  fs::remove_dir_all(&path).expect("delete the run's worktree by hand");
  assert!(!path.exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_worktree_paused_run_cannot_resume_and_remains_unchanged() {
  let project = project(&Setup {
    review: true,
    ..Setup::new("Work slowly.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = start(&scheduler, project.task).await;
  wait_for_turns(root, &mut events, id, 1).await;
  tokio::time::timeout(PATIENCE, scheduler.pause_run(id))
    .await
    .expect("pause request answered in time")
    .expect("pause requested");
  let paused = wait_for(root, &mut events, id, |run| {
    run.status() == &RunStatus::Paused(PauseReason::Requested)
  })
  .await;
  assert!(paused.last_checkpoint().is_some());
  wait_for_idle(&scheduler).await;

  let before = stored(root, id);
  delete_worktree(root, id);
  assert_eq!(scheduler.resume_run(id).await, Err(Rejection::WorktreeRemoved));
  assert_eq!(
    scheduler.resume_run(RunId::generate()).await,
    Err(Rejection::RunNotFound)
  );
  assert_eq!(stored(root, id), before, "rejection cannot resume or append events");
  assert!(scheduler.active_runs().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_worktree_interrupted_run_cannot_continue_and_remains_unchanged() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Work in turns.")
  });
  let root = project.repo.path();
  let (stopping, mut events) = scheduler(root);
  let id = start(&stopping, project.task).await;
  wait_for_turns(root, &mut events, id, 2).await;
  stopping.stop_all(InterruptCause::QuitNow).await;
  let before = stored(root, id);
  assert_eq!(before.0.status(), &RunStatus::Interrupted(InterruptCause::QuitNow));

  delete_worktree(root, id);
  let (continuing, _) = scheduler(root);
  let unknown = RunId::generate();
  let (continued, refused) = continuing.continue_runs(&[unknown, id]).await;
  assert!(continued.is_empty());
  assert_eq!(refused.len(), 2);
  assert_eq!(refused[0].run, unknown);
  assert_eq!(refused[0].rejection, Rejection::RunNotFound);
  assert_eq!(refused[1].run, id);
  assert_eq!(refused[1].rejection, Rejection::WorktreeRemoved);
  assert_eq!(stored(root, id), before, "rejection cannot continue or append events");
  assert!(continuing.active_runs().is_empty());
}
