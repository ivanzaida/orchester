//! Recovery exercises real scheduler, persistence, worktrees and fake provider processes.
mod support;

use std::{fs, path::Path};

use orchester_core::{
  conversation::{Delivery, Message, MessageBody, Recipient},
  id::{IdempotencyKey, RunId},
  resource::StepKey,
  run::{AttemptReason, EventSequence, Run, RunStatus},
  store::{ActivityStore, ConversationStore, RunStore},
  time::Timestamp,
  tokens::TokensUsed,
};
use orchester_ipc::AttachSession;
use orchester_store::ProjectDb;
use support::{Setup, StepSpec, git, project, reported, scheduler, tools::ToolSpec, wait_for, wait_for_idle};

fn read(root: &Path, id: RunId) -> Run {
  ProjectDb::open(root)
    .expect("open")
    .run(id)
    .expect("read")
    .expect("run")
}

fn send(root: &Path, run: &Run, text: &str, marker: Option<EventSequence>) -> Message {
  // Deliberately use the exact failure timestamp: ordering must not depend on milliseconds.
  let now = ProjectDb::open(root)
    .expect("open")
    .events(run.id())
    .expect("events")
    .last()
    .expect("event")
    .at();
  let message = Message::from_person(
    run.task(),
    Some(run.id()),
    Recipient::Orchestrator,
    MessageBody::new(text).expect("body"),
    now,
  );
  let message = match marker {
    Some(sequence) => message.recovering_after(sequence).expect("recovery message"),
    None => message,
  };
  ProjectDb::open(root)
    .expect("open")
    .send_message(IdempotencyKey::generate(), &message)
    .expect("stored")
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_recovery_preserves_work_session_history_and_delivers_once() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let failed = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  assert!(matches!(failed.status(), RunStatus::Failed(_)));
  let path = root.join(".orchester/worktrees").join(id.to_string());
  fs::write(path.join("keep-uncommitted.txt"), "keep me").expect("uncommitted work");
  let message = send(
    root,
    &failed,
    "Continue with this correction.",
    Some(failed.last_sequence()),
  );
  let (first, duplicate) = tokio::join!(scheduler.deliver_messages(id), scheduler.deliver_messages(id));
  first.expect("continue");
  duplicate.expect("duplicate notification");
  let done = wait_for(root, &mut events, id, |run| run.status() == &RunStatus::Succeeded).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(done.id(), failed.id());
  assert_eq!(done.branch(), failed.branch());
  assert_eq!(done.attempts().len(), 2);
  assert_eq!(done.attempts()[0], failed.attempts()[0]);
  assert!(matches!(
    done.current_attempt().expect("attempt").reason(),
    AttemptReason::Recovery { .. }
  ));
  assert_eq!(
    fs::read_to_string(path.join("README.md")).expect("work"),
    "hi\nhello\ncontinued\n"
  );
  assert_eq!(
    git(root, &["show", &format!("{}:keep-uncommitted.txt", done.branch())]),
    "keep me"
  );
  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 2);
  assert_eq!(invocations[0].session.as_deref(), Some("fake-thread-1"));
  assert_eq!(invocations[1].session, invocations[0].session);
  let prompt = reported(&db, id, "prompt| ");
  assert!(prompt.iter().any(|line| line == "Continue with this correction."));
  assert!(prompt.iter().any(|line| line.contains("Finished Implement")));
  let stored = db.messages(project.task).expect("messages");
  assert!(matches!(
    stored
      .iter()
      .find(|stored| stored.id() == message.id())
      .expect("message")
      .delivery(),
    Some(Delivery::Acknowledged { .. })
  ));
  assert_eq!(db.runs_of_task(project.task).expect("runs").len(), 1);
  assert_eq!(Run::replay(&db.events(id).expect("events")).expect("replay"), done);
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_recovery_a_message_continues_a_run_that_waits_for_the_person() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Append a line.", &[]),
      StepSpec::new("review", "Review", "Report failure.", &["implement"]).retry("implement", 1),
    ],
    ..Setup::new("Say hello.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let waiting = wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::WaitingForPerson(_))
  })
  .await;
  wait_for_idle(&scheduler).await;
  // A message queued without the choice to continue keeps waiting with the run.
  send(root, &waiting, "An ordinary queued message.", None);
  scheduler.deliver_messages(id).await.expect("stays queued");
  assert_eq!(read(root, id), waiting);
  let message = send(root, &waiting, "So what do you suggest?", Some(waiting.last_sequence()));
  scheduler.deliver_messages(id).await.expect("continue");
  let done = wait_for(root, &mut events, id, |run| run.status() == &RunStatus::Succeeded).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(done.attempts().len(), 3);
  assert!(matches!(
    done.current_attempt().expect("attempt").reason(),
    AttemptReason::Recovery { .. }
  ));
  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(id).expect("invocations");
  // The step that failed continues its own session: implement, review, both again, then review.
  assert_eq!(invocations.len(), 5);
  assert_eq!(invocations[4].session, invocations[3].session);
  let delivered = db.messages(project.task).expect("messages");
  assert!(matches!(
    delivered
      .iter()
      .find(|stored| stored.id() == message.id())
      .expect("message")
      .delivery(),
    Some(Delivery::Acknowledged { .. })
  ));
  assert_eq!(Run::replay(&db.events(id).expect("events")).expect("replay"), done);
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_recovery_stale_messages_cannot_retry_a_new_failure() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let failed = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  send(root, &failed, "An ordinary queued message.", None);
  scheduler.deliver_messages(id).await.expect("stays queued");
  assert_eq!(read(root, id), failed);
  send(
    root,
    &failed,
    "Please fail this resumed turn.",
    Some(failed.last_sequence()),
  );
  scheduler.deliver_messages(id).await.expect("recover");
  // The resumed turn breaks, so its step restarts once as a fresh session, which reads the brief
  // and reports the failure again.
  let again = wait_for(root, &mut events, id, |run| {
    run.attempts().len() == 3 && run.status().is_terminal()
  })
  .await;
  wait_for_idle(&scheduler).await;
  assert!(matches!(again.status(), RunStatus::Failed(_)));
  send(
    root,
    &again,
    "Sent from a stale composer.",
    Some(failed.last_sequence()),
  );
  scheduler
    .deliver_messages(id)
    .await
    .expect("stale message stays queued");
  assert_eq!(read(root, id), again);
  send(root, &again, "Now continue successfully.", Some(again.last_sequence()));
  scheduler.deliver_messages(id).await.expect("fresh recovery");
  let done = wait_for(root, &mut events, id, |run| run.status() == &RunStatus::Succeeded).await;
  assert_eq!(done.attempts().len(), 4);
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_recovery_offline_message_and_late_usage_survive_attach() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  let (old, mut events) = scheduler(root);
  let id = old
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let mut failed = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&old).await;
  drop(old);
  let usage = failed
    .record_usage(TokensUsed::new(1), Timestamp::now())
    .expect("late usage");
  ProjectDb::open(root)
    .expect("open")
    .append_events(&failed, &usage)
    .expect("saved usage");
  send(root, &failed, "Continue after reconnect.", Some(failed.last_sequence()));
  let (next, mut events) = scheduler(root);
  next
    .attach(AttachSession::Reconnect, std::num::NonZeroU32::MIN)
    .await
    .expect("attach");
  let done = wait_for(root, &mut events, id, |run| run.status() == &RunStatus::Succeeded).await;
  assert_eq!(done.attempts().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_recovery_removed_worktree_leaves_history_and_message_queued() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let failed = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  let path = root.join(".orchester/worktrees").join(id.to_string());
  git(root, &["worktree", "remove", path.to_str().expect("path")]);
  let message = send(root, &failed, "Please continue.", Some(failed.last_sequence()));
  assert!(scheduler.deliver_messages(id).await.is_err());
  assert_eq!(read(root, id), failed);
  assert!(!path.exists());
  assert_eq!(
    ProjectDb::open(root)
      .expect("open")
      .messages(project.task)
      .expect("messages")
      .iter()
      .find(|stored| stored.id() == message.id())
      .expect("stored")
      .delivery(),
    Some(Delivery::Queued)
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_recovery_command_failure_resumes_upstream_agent_and_reruns_checks() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Append a line.", &[]),
      StepSpec::new("checks", "Checks", "", &["implement"])
        .command(ToolSpec::test_command(&["fail-once", "check-marker"])),
    ],
    ..Setup::new("Say hello.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let failed = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  assert!(matches!(failed.status(), RunStatus::Failed(_)));
  assert_eq!(
    failed.recovery_step(None),
    Some(StepKey::new("implement").expect("step"))
  );
  send(
    root,
    &failed,
    "Fix the issue and run the checks again.",
    Some(failed.last_sequence()),
  );
  scheduler.deliver_messages(id).await.expect("recover");
  let done = wait_for(root, &mut events, id, |run| run.status() == &RunStatus::Succeeded).await;
  let calls = ProjectDb::open(root).expect("open").invocations(id).expect("calls");
  assert_eq!(calls.len(), 4, "agent and command run twice");
  assert_eq!(done.attempts().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_recovery_parallel_worktree_is_reused_and_removed_after_merge() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Write own file.", &[]),
      StepSpec::new(
        "checks",
        "Checks",
        "Write own file. Report failure. Work slowly.",
        &["implement"],
      ),
      StepSpec::new("review", "Review", "Write own file.", &["implement"]),
    ],
    ..Setup::new("Say hello.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let failed = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  let execution = failed
    .latest_execution(&StepKey::new("checks").expect("key"))
    .expect("checks")
    .id();
  let path = root.join(".orchester/worktrees").join(execution.to_string());
  assert!(path.is_dir(), "failed parallel work survives");
  fs::write(path.join("partial.txt"), "partial check fix").expect("partial work");
  send(
    root,
    &failed,
    "Continue the failed check agent.",
    Some(failed.last_sequence()),
  );
  scheduler.deliver_messages(id).await.expect("recover");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  wait_for_idle(&scheduler).await;
  assert!(!path.exists(), "original owner worktree removed after successful merge");
  assert_eq!(
    git(root, &["show", &format!("{}:partial.txt", done.branch())]),
    "partial check fix"
  );
  assert_eq!(git(root, &["show", &format!("{}:checks.txt", done.branch())]), "hello");
  assert_eq!(git(root, &["branch", "--list", &format!("{}--*", done.branch())]), "");
}
