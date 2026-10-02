//! Migrates a waiting exec session into App Server without replacing its run or worktree.
mod support;

use std::path::Path;

use orchester_core::{
  conversation::{Message, MessageBody, Recipient},
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, CommandPermission, FolderBounds, LaunchPermissions, WritableFolder, WritableFolders},
  resource::{AgentProfile, StepKey},
  run::{AttemptReason, PendingToolApproval, PermissionsApply, RunStatus},
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
  let permissions = migration_permissions(root, profile);
  let old_policies = policies(&db, id);
  scheduler
    .change_permissions(id, profile, permissions.clone(), PermissionsApply::NextInvocation)
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
  assert_migration_policies(&db, id, &old_policies, &permissions);
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

fn policies(db: &ProjectDb, run: RunId) -> Vec<Option<LaunchPermissions>> {
  db.invocations(run)
    .expect("invocations")
    .into_iter()
    .map(|invocation| invocation.permissions)
    .collect()
}

fn assert_migration_policies(
  db: &ProjectDb,
  run: RunId,
  old: &[Option<LaunchPermissions>],
  desired: &AgentPermissions,
) {
  let actual = policies(db, run);
  assert_eq!(&actual[..2], old);
  let enforced = desired
    .enforced_on(
      orchester_core::connection::AgentProvider::CodexCli,
      orchester_core::connection::invocation::Platform::current(),
    )
    .expect("actual supported policy");
  assert_eq!(
    actual[2],
    Some(enforced.launch()),
    "actual App Server policy includes Ask and MCP approvals"
  );
}

fn migration_permissions(root: &Path, profile: orchester_core::id::ResourceId) -> AgentPermissions {
  let db = ProjectDb::open(root).expect("open");
  let published = db
    .latest_version::<AgentProfile>(profile)
    .expect("version")
    .expect("profile");
  let mut permissions = published.body().permissions().clone();
  permissions.set_commands(CommandPermission::AskBefore);
  permissions.set_interactive_tool_approvals(true);
  let bounds = FolderBounds::new(root, None).expect("folder bounds");
  permissions.with_writable_folders(
    WritableFolders::new(vec![
      WritableFolder::new(".orchester/extra", &bounds).expect("extra folder"),
    ])
    .expect("folder list"),
  )
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

#[tokio::test(flavor = "multi_thread")]
async fn command_approvals_soft_stop_after_delivery_prevents_automatic_replay() {
  let project = project(&Setup {
    steps: vec![StepSpec::new("implement", "Implement", "Ask first.", &[])],
    ..Setup::new("Continue the existing work.")
  });
  let root = project.repo.path();
  let (first, mut events) = scheduler(root);
  let id = first
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let before = wait_for(root, &mut events, id, |run| run.waits_for_a_person()).await;
  wait_for_idle(&first).await;
  let mut db = ProjectDb::open(root).expect("open");
  let profile = before
    .profile_of(&StepKey::new("implement").expect("key"))
    .expect("profile");
  let mut permissions = db
    .latest_version::<AgentProfile>(profile)
    .expect("version")
    .expect("profile")
    .body()
    .permissions()
    .clone();
  permissions.set_commands(CommandPermission::AskBefore);
  first
    .change_permissions(id, profile, permissions, PermissionsApply::NextInvocation)
    .await
    .expect("permissions");
  let message = Message::from_person(
    project.task,
    Some(id),
    Recipient::Orchestrator,
    MessageBody::new("Hold approved command.").expect("body"),
    Timestamp::now(),
  );
  db.send_message(IdempotencyKey::generate(), &message).expect("message");
  first.deliver_messages(id).await.expect("resume");
  let command = pending(root, &mut events, id).await;
  first.decide_tool_approval(id, command.id, true).await.expect("approve");
  let marker = root
    .join(".orchester/worktrees")
    .join(id.to_string())
    .join("command-once.txt");
  tokio::time::timeout(PATIENCE, async {
    while !marker.exists() {
      tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
  })
  .await
  .expect("approved action starts");
  first.stop_all(orchester_core::run::InterruptCause::RunnerLost).await;
  wait_for_idle(&first).await;
  assert!(db.has_interrupted_tool_approval(id).expect("interruption"));
  let invocations = db.invocations(id).expect("invocations");
  assert!(invocations.last().expect("last").ended_at.is_some());
  let (second, _) = scheduler(root);
  let attached = second
    .attach(
      orchester_ipc::AttachSession::Reconnect,
      std::num::NonZeroU32::new(1).expect("slot"),
    )
    .await
    .expect("reconnect");
  assert!(attached.continued.is_empty());
  assert!(second.active_runs().is_empty());
  assert_eq!(db.invocations(id).expect("invocations").len(), invocations.len());
  assert_eq!(std::fs::read_to_string(marker).expect("marker"), "approved");
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
  let denied = deny_command || deny_tool;
  let after = wait_for(root, events, id, |run| {
    if denied {
      run.waits_for_a_person() && run.attempts().len() == 2
    } else {
      run.status().is_terminal()
    }
  })
  .await;
  if denied {
    assert!(matches!(
      after.current_attempt().map(|attempt| attempt.reason()),
      Some(AttemptReason::Restart { step }) if step.as_str() == "implement"
    ));
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
  assert_eq!(invocations.len(), if denied { 4 } else { 3 });
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
