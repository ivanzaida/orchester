//! Permission restarts must not replay an approved MCP action whose completion is unknown.
mod support;

use std::{path::Path, time::Duration};

use orchester_core::{
  conversation::{Message, MessageBody, Recipient},
  id::{IdempotencyKey, RunId},
  permission::CommandPermission,
  resource::{AgentProfile, StepKey},
  run::{Answerer, ApprovalAnswer, PermissionsApply, RunStatus},
  store::{ActivityStore, ConversationStore, ResourceStore},
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::ProjectDb;
use support::{PATIENCE, Setup, StepSpec, project, scheduler, wait_for, wait_for_idle};
use tokio::sync::mpsc;

async fn pending(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) -> String {
  tokio::time::timeout(PATIENCE, async {
    loop {
      let db = ProjectDb::open(root).expect("open");
      if let Some(request) = db.pending_tool_approvals(run).expect("requests").first() {
        return request.id.clone();
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("approval arrives")
}

async fn permission_restart(hold_mcp: bool) {
  let project = project(&Setup {
    steps: vec![StepSpec::new("implement", "Implement", "Ask first.", &[])],
    ..Setup::new("Continue the existing work.")
  });
  let root = project.repo.path();
  let (runner, mut events) = scheduler(root);
  let id = runner
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let before = wait_for(root, &mut events, id, |run| run.waits_for_a_person()).await;
  wait_for_idle(&runner).await;
  let mut db = ProjectDb::open(root).expect("open");
  let profile = before
    .profile_of(&StepKey::new("implement").expect("key"))
    .expect("profile");
  let permissions = interactive_permissions(&db, profile);
  runner
    .change_permissions(id, profile, permissions.clone(), PermissionsApply::NextInvocation)
    .await
    .expect("permissions");
  let message = Message::from_person(
    project.task,
    Some(id),
    Recipient::Orchestrator,
    MessageBody::new("Hold approved MCP.").expect("body"),
    Timestamp::now(),
  );
  db.send_message(IdempotencyKey::generate(), &message).expect("message");
  runner.deliver_messages(id).await.expect("resume");
  let command = pending(root, &mut events, id).await;
  runner
    .decide_tool_approval(id, command, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approve command");
  let tool = pending(root, &mut events, id).await;
  let marker = root
    .join(".orchester/worktrees")
    .join(id.to_string())
    .join("mcp-once.txt");
  if hold_mcp {
    runner
      .decide_tool_approval(id, tool, ApprovalAnswer::Once, Answerer::Person)
      .await
      .expect("approve MCP");
    wait_for_marker(&marker).await;
  }
  let count = db.invocations(id).expect("invocations").len();
  runner
    .change_permissions(id, profile, permissions, PermissionsApply::Now)
    .await
    .expect("apply now");
  if hold_mcp {
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(db.invocations(id).expect("invocations").len(), count);
    assert!(
      db.invocations(id)
        .expect("invocations")
        .last()
        .expect("last")
        .ended_at
        .is_none()
    );
    assert_eq!(std::fs::read_to_string(&marker).expect("side effect"), "approved");
    runner.stop_all(orchester_core::run::InterruptCause::RunnerLost).await;
  } else {
    let paused = wait_for(root, &mut events, id, |run| {
      matches!(run.status(), RunStatus::Paused(_))
    })
    .await;
    assert_eq!(paused.id(), id);
    assert!(!marker.exists());
  }
  wait_for_idle(&runner).await;
  assert!(db.has_interrupted_tool_approval(id).expect("uncertainty recorded"));
  assert_eq!(db.invocations(id).expect("invocations").len(), count);
}

fn interactive_permissions(
  db: &ProjectDb,
  profile: orchester_core::id::ResourceId,
) -> orchester_core::permission::AgentPermissions {
  let mut permissions = db
    .latest_version::<AgentProfile>(profile)
    .expect("version")
    .expect("profile")
    .body()
    .permissions()
    .clone();
  permissions.set_commands(CommandPermission::AskBefore);
  permissions.set_interactive_tool_approvals(true);
  permissions
}

async fn wait_for_marker(marker: &Path) {
  tokio::time::timeout(PATIENCE, async {
    while !marker.exists() {
      tokio::time::sleep(Duration::from_millis(10)).await;
    }
  })
  .await
  .expect("MCP action starts");
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_permissions_wait_for_active_mcp_completion() {
  permission_restart(true).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_permissions_pause_instead_of_automatic_uncertain_resume() {
  permission_restart(false).await;
}
