//! Migrates a waiting exec session into App Server without replacing its run or worktree.
mod support;

use std::path::Path;

use orchester_core::{
  conversation::{Message, MessageBody, Recipient},
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, CommandPermission},
  resource::{AgentProfile, StepKey},
  run::{PendingToolApproval, PermissionsApply, RunStatus},
  store::{ActivityStore, ConversationStore, ResourceStore},
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::ProjectDb;
use support::{PATIENCE, Setup, StepSpec, project, scheduler, wait_for, wait_for_idle};
use tokio::sync::mpsc;

async fn pending(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) -> PendingToolApproval {
  tokio::time::timeout(PATIENCE, async {
    loop {
      let db = ProjectDb::open(root).expect("open");
      if let Some(request) = db.pending_tool_approvals(run).expect("requests").first()
        && db
          .invocations(run)
          .expect("invocations")
          .iter()
          .any(|invocation| invocation.id == request.invocation && invocation.timing.waiting_since().is_some())
      {
        return request.clone();
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("approval arrives")
}

async fn migrated(deny_command: bool, deny_tool: bool) {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("prepare", "Prepare", "Say hello.", &[]),
      StepSpec::new("implement", "Implement", "Ask first.", &["prepare"]),
    ],
    ..Setup::new("Continue the existing work.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let before = wait_for(root, &mut events, id, |run| run.waits_for_a_person()).await;
  wait_for_idle(&scheduler).await;
  let step = StepKey::new("implement").expect("key");
  let profile = before.profile_of(&step).expect("profile");
  let db = ProjectDb::open(root).expect("open");
  let published = db
    .latest_version::<AgentProfile>(profile)
    .expect("version")
    .expect("profile");
  let mut permissions: AgentPermissions = published.body().permissions().clone();
  permissions.set_commands(CommandPermission::AskBefore);
  permissions.set_interactive_tool_approvals(true);
  scheduler
    .change_permissions(id, profile, permissions, PermissionsApply::NextInvocation)
    .await
    .expect("change");
  let worktree = root.join(".orchester/worktrees").join(id.to_string());
  std::fs::write(worktree.join("uncommitted.txt"), "keep progress").expect("uncommitted work");
  let message = Message::from_person(
    project.task,
    Some(id),
    Recipient::Orchestrator,
    MessageBody::new("Continue with approvals.").expect("body"),
    Timestamp::now(),
  );
  ProjectDb::open(root)
    .expect("open")
    .send_message(IdempotencyKey::generate(), &message)
    .expect("message");
  scheduler.deliver_messages(id).await.expect("same session continues");
  let command = pending(root, &mut events, id).await;
  assert!(command.command);
  assert_eq!(command.target.as_deref(), worktree.to_str());
  assert!(!worktree.join("command-once.txt").exists());
  assert_eq!(
    scheduler
      .decide_tool_approval(id, "different-request".to_owned(), true)
      .await,
    Err(Rejection::ToolApprovalNotAwaited)
  );
  scheduler
    .decide_tool_approval(id, command.id.clone(), !deny_command)
    .await
    .expect("command decision");
  assert_eq!(
    scheduler.decide_tool_approval(id, command.id, true).await,
    Err(Rejection::ToolApprovalNotAwaited)
  );
  if !deny_command {
    let tool = pending(root, &mut events, id).await;
    assert!(!tool.command);
    assert!(!worktree.join("mcp-once.txt").exists());
    assert!(
      db.invocations(id).expect("invocations")[2]
        .timing
        .waiting_since()
        .is_some()
    );
    scheduler
      .decide_tool_approval(id, tool.id, !deny_tool)
      .await
      .expect("tool decision");
  }
  verify_progress(root, &mut events, id, &before, &message, (deny_command, deny_tool)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn command_approvals_migrate_waiting_exec_session_and_approve_each_action() {
  migrated(false, false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn command_approvals_deny_command_without_side_effects() {
  migrated(true, false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn command_approvals_deny_mcp_write_without_side_effects() {
  migrated(false, true).await;
}

async fn verify_progress(
  root: &Path,
  events: &mut mpsc::UnboundedReceiver<Event>,
  id: RunId,
  before: &orchester_core::run::Run,
  message: &Message,
  (deny_command, deny_tool): (bool, bool),
) {
  let db = ProjectDb::open(root).expect("open");
  let worktree = root.join(".orchester/worktrees").join(id.to_string());
  let profile = before
    .profile_of(&StepKey::new("implement").expect("key"))
    .expect("profile");
  let after = wait_for(root, events, id, |run| run.status().is_terminal()).await;
  if deny_command || deny_tool {
    assert!(matches!(after.status(), RunStatus::Failed(_)));
  } else {
    assert_eq!(after.status(), &RunStatus::Succeeded);
  }
  assert_eq!(after.id(), before.id());
  assert_eq!(after.branch(), before.branch());
  assert_eq!(after.snapshot(), before.snapshot());
  assert_eq!(
    std::fs::read_to_string(worktree.join("uncommitted.txt")).expect("progress"),
    "keep progress"
  );
  assert_eq!(worktree.join("command-once.txt").exists(), !deny_command);
  assert_eq!(worktree.join("mcp-once.txt").exists(), !deny_command && !deny_tool);
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 3);
  assert_eq!(invocations[1].session, invocations[2].session);
  assert_eq!(invocations[1].execution, invocations[2].execution);
  let prepared = StepKey::new("prepare").expect("key");
  assert_eq!(before.latest_execution(&prepared), after.latest_execution(&prepared));
  assert!(
    !db
      .latest_version::<AgentProfile>(profile)
      .expect("version")
      .expect("profile")
      .body()
      .permissions()
      .interactive_tool_approvals()
  );
  assert!(
    db.messages(message.task())
      .expect("conversation")
      .iter()
      .any(|entry| entry.id() == message.id())
  );
}
