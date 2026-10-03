//! A graceful shutdown saves waiting sessions without approving an action or killing active work.
mod support;

use std::{path::Path, time::Duration};

use orchester_core::{
  conversation::{Message, MessageBody, Recipient},
  id::{IdempotencyKey, RunId, TaskId},
  permission::CommandPermission,
  resource::{AgentProfile, StepKey},
  run::{ApprovalAnswer, PauseReason, PendingToolApproval, PermissionsApply, RunStatus, StepStatus},
  store::{ActivityStore, ConversationStore, ResourceStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::{AttachSession, Event};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use support::{PATIENCE, Setup, StepSpec, git, project, scheduler, wait_for, wait_for_idle};
use tokio::sync::mpsc;

async fn pending(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) -> PendingToolApproval {
  tokio::time::timeout(PATIENCE, async {
    loop {
      let db = ProjectDb::open(root).expect("open");
      if let Some(request) = db.pending_tool_approvals(run).expect("requests").first() {
        return request.clone();
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("approval arrives")
}

async fn shutdown_at_approval(approve_command: bool, pause_while_active: bool) {
  let project = project(&Setup {
    steps: vec![StepSpec::new("implement", "Implement", "Ask first.", &[])],
    ..Setup::new("Continue the existing work.")
  });
  let root = project.repo.path();
  let (runner, mut events) = scheduler(root);
  let id = continue_interactive(root, &runner, &mut events, project.task, pause_while_active).await;
  let db = ProjectDb::open(root).expect("open");
  let step = StepKey::new("implement").expect("key");
  let command = pending(root, &mut events, id).await;
  let worktree = root.join(".orchester/worktrees").join(id.to_string());
  std::fs::write(worktree.join("progress.txt"), "keep actual progress").expect("work");
  if approve_command {
    runner
      .decide_tool_approval(id, command.id, ApprovalAnswer::Once)
      .await
      .expect("approve command");
    if !pause_while_active {
      assert!(!pending(root, &mut events, id).await.command);
    } else {
      wait_for_marker(&worktree.join("command-once.txt")).await;
    }
  }
  let finishing = tokio::spawn({
    let runner = runner.clone();
    async move { runner.finish_at_checkpoints().await }
  });
  if pause_while_active {
    wait_for(root, &mut events, id, |run| run.pause_requested()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(!finishing.is_finished(), "the approved command must finish");
    assert!(
      db.invocations(id)
        .expect("invocations")
        .last()
        .expect("last")
        .ended_at
        .is_none()
    );
    std::fs::write(worktree.join("release-active.txt"), "release").expect("release command");
  }
  tokio::time::timeout(PATIENCE, finishing)
    .await
    .expect("shutdown settles")
    .expect("shutdown task");
  verify_saved(root, id, &step, approve_command);
  let (reopened, _) = scheduler(root);
  let attached = reopened
    .attach(AttachSession::Reconnect, std::num::NonZeroU32::new(1).expect("slot"))
    .await
    .expect("reconnect");
  assert!(attached.continued.is_empty());
  assert!(reopened.active_runs().is_empty());
  assert_eq!(db.invocations(id).expect("preserved invocations").len(), 2);
}

async fn continue_interactive(
  root: &Path,
  runner: &Scheduler,
  events: &mut mpsc::UnboundedReceiver<Event>,
  task: TaskId,
  pause_while_active: bool,
) -> RunId {
  let id = runner.start_run(IdempotencyKey::generate(), task).await.expect("start");
  let before = wait_for(root, events, id, |run| run.waits_for_a_person()).await;
  wait_for_idle(runner).await;
  let mut db = ProjectDb::open(root).expect("open");
  let step = StepKey::new("implement").expect("key");
  let profile = before.profile_of(&step).expect("profile");
  let mut permissions = db
    .latest_version::<AgentProfile>(profile)
    .expect("version")
    .expect("profile")
    .body()
    .permissions()
    .clone();
  permissions.set_commands(CommandPermission::AskBefore);
  permissions.set_interactive_tool_approvals(true);
  runner
    .change_permissions(id, profile, permissions, PermissionsApply::NextInvocation)
    .await
    .expect("permissions");
  let text = if pause_while_active {
    "Release approved command."
  } else {
    "Continue with approvals."
  };
  let message = Message::from_person(
    task,
    Some(id),
    Recipient::Orchestrator,
    MessageBody::new(text).expect("body"),
    Timestamp::now(),
  );
  db.send_message(IdempotencyKey::generate(), &message).expect("message");
  runner.deliver_messages(id).await.expect("resume");
  id
}

fn verify_saved(root: &Path, id: RunId, step: &StepKey, approve_command: bool) {
  let db = ProjectDb::open(root).expect("open");
  let paused = db.run(id).expect("read").expect("run");
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::Requested));
  assert_eq!(
    paused.latest_execution(step).expect("execution").status(),
    StepStatus::Running
  );
  let checkpoint = paused.last_checkpoint().expect("progress checkpoint");
  assert_eq!(
    git(root, &["show", &format!("{}:progress.txt", checkpoint.revision())]),
    "keep actual progress"
  );
  let worktree = root.join(".orchester/worktrees").join(id.to_string());
  assert_eq!(worktree.join("command-once.txt").exists(), approve_command);
  assert!(!worktree.join("mcp-once.txt").exists());
  assert!(db.pending_tool_approvals(id).expect("pending approvals").is_empty());
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 2);
  assert_eq!(invocations[1].session.as_deref(), Some("fake-thread-1"));
  assert!(invocations[1].ended_at.is_some());
  assert!(invocations[1].timing.waiting_since().is_none());
}

async fn wait_for_marker(marker: &Path) {
  tokio::time::timeout(PATIENCE, async {
    while !marker.exists() {
      tokio::time::sleep(Duration::from_millis(10)).await;
    }
  })
  .await
  .expect("command starts");
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_shutdown_saves_pending_command_without_granting_it() {
  shutdown_at_approval(false, false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_shutdown_saves_pending_mcp_without_granting_it() {
  shutdown_at_approval(true, false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_shutdown_waits_for_active_command_then_saves_later_pending_mcp() {
  shutdown_at_approval(true, true).await;
}
