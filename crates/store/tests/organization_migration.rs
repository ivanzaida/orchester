//! Migration 39, task organization: a populated schema 38 database opens with all it held, and
//! existing tasks get the status their runs imply.
mod support;

use orchester_core::{
  id::IdempotencyKey,
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::BranchName,
  store::{MergeStore, RunStore, TaskStore},
  task::organization::{ChangeActor, Priority, TaskOrganization, WorkflowStatus},
};
use orchester_store::ProjectDb;
use support::{at, copied_at_version, started_run};

/// The schema version before tasks had an organization: master's last migrations, ORC-61's
/// steering (37) and the approval choices (38). Migrations are append-only.
const BEFORE_ORGANIZATION: u32 = 38;

/// A populated project database of schema 38 (a message sent now, an approval rule, a run's own
/// auto-approval choice) opens with all of it, each task in Backlog with no priority, tags or
/// links, and takes organization changes.
#[test]
fn organization_migration_keeps_tasks_and_runs_of_schema_38() {
  use orchester_core::{
    conversation::{Message, MessageBody, Recipient, Timing},
    run::RuleScope,
    store::ConversationStore,
  };
  let (repo, mut db, run, _) = started_run();
  let now = Message::from_person(
    run.task(),
    Some(run.id()),
    Recipient::Orchestrator,
    MessageBody::new("Use the new API.").expect("a message"),
    at(12),
  )
  .sent(Timing::Now)
  .expect("a person's message");
  db.send_message(IdempotencyKey::generate(), &now).expect("sent now");
  let scope = RuleScope::SimilarMcpTool {
    server: "pencil".to_owned(),
    tool: "batch_get".to_owned(),
    folder: None,
  };
  db.add_approval_rule(run.id(), &scope, at(13))
    .expect("an approval rule");
  db.set_run_auto_approve(run.id(), Some(true)).expect("auto-approve");
  drop(db);
  let current = repo.path().join(".orchester").join("orchester.db");
  let old = copied_at_version(&current, BEFORE_ORGANIZATION);
  let mut db = ProjectDb::open(old.path()).expect("migrate existing project");
  let task = run.task();
  assert_eq!(db.run(run.id()).expect("read").expect("run"), run);
  assert_eq!(db.messages(task).expect("messages"), [now]);
  assert_eq!(db.approval_rules(run.id()).expect("rules").len(), 1);
  assert_eq!(db.run_auto_approve(run.id()).expect("read"), Some(true));
  // Its run has not ended: the migration sets the task In progress.
  assert_eq!(
    db.task_organization(task).expect("organization"),
    TaskOrganization::default().with_status(WorkflowStatus::InProgress)
  );
  let value = TaskOrganization::default().with_priority(Priority::High);
  db.set_task_organization(task, &value, ChangeActor::Person, at(20))
    .expect("organize a migrated task");
  assert_eq!(db.task_organization(task).expect("organization"), value);
}

/// How the run fixture is left before the database goes back to schema 38.
#[derive(Clone, Copy, Debug)]
enum Before {
  Running,
  ReadyForReview,
  Merged,
  /// The run was cancelled and the task is a draft again.
  CancelledDraft,
  /// The run was cancelled and the task stays queued.
  CancelledQueued,
  Paused,
  Interrupted,
  /// The step broke twice and the run waits for a person's decision.
  Waiting,
  /// The run failed and the task stays queued.
  FailedQueued,
  /// The run failed and the task is a draft again.
  FailedDraft,
}

/// Leaves the fixture's run paused, interrupted, waiting for a person or failed for `before`; the
/// other states are
/// reached in [`backfilled`].
fn prepare(
  db: &mut ProjectDb,
  run: &mut orchester_core::run::Run,
  execution: orchester_core::id::StepExecutionId,
  before: Before,
) {
  use orchester_core::run::{InterruptCause, RunStatus, StepFailure};
  let events = match before {
    Before::Paused => run.pause(at(21)).expect("pause"),
    Before::Interrupted => run.interrupt(InterruptCause::RunnerLost, at(21)).expect("interrupt"),
    Before::FailedQueued | Before::FailedDraft => {
      let events = run.start_step(execution, at(21)).expect("start step");
      db.append_events(run, &events).expect("stored");
      run
        .fail_step(execution, StepFailure::ReportedFailed, at(22))
        .expect("fail step")
    }
    Before::Waiting => {
      for millis in [21, 23] {
        let execution = run.current_attempt().expect("attempt").executions()[0].id();
        let events = run.start_step(execution, at(millis)).expect("start step");
        db.append_events(run, &events).expect("stored");
        let events = run
          .fail_step(execution, StepFailure::AgentFailed, at(millis + 1))
          .expect("fail step");
        db.append_events(run, &events).expect("stored");
      }
      assert!(matches!(run.status(), RunStatus::WaitingForPerson(_)));
      return;
    }
    _ => return,
  };
  db.append_events(run, &events).expect("stored");
  if matches!(before, Before::FailedQueued | Before::FailedDraft) {
    assert!(matches!(run.status(), RunStatus::Failed(_)), "{:?}", run.status());
  }
}

/// The status migration 39 gives the fixture's task after `before`, and a draft (Backlog) and a
/// queued task without a run (Todo) beside it.
fn backfilled(before: Before) -> WorkflowStatus {
  let (repo, mut db, mut run, execution) = started_run();
  let details = db.task(run.task()).expect("read").expect("task").details().clone();
  let others = db
    .create_tasks(IdempotencyKey::generate(), vec![details.clone(), details], at(20))
    .expect("tasks");
  prepare(&mut db, &mut run, execution, before);
  if matches!(before, Before::ReadyForReview | Before::Merged) {
    let events = run.start_step(execution, at(21)).expect("start step");
    db.append_events(&run, &events).expect("stored");
    let events = run.complete_step(execution, at(22)).expect("complete step");
    db.append_events(&run, &events).expect("stored");
  }
  if matches!(before, Before::Merged) {
    db.save_run_merge(&RunMerge {
      run: run.id(),
      into: BranchName::new("main").expect("branch"),
      revision: Revision::new(&"c".repeat(40)).expect("revision"),
      kind: MergeKind::FastForward,
      merged_at: at(23),
    })
    .expect("merge");
  }
  if matches!(before, Before::CancelledDraft | Before::CancelledQueued) {
    let events = run.cancel(at(21)).expect("cancel");
    db.append_events(&run, &events).expect("stored");
  }
  drop(db);
  let old = copied_at_version(
    &repo.path().join(".orchester").join("orchester.db"),
    BEFORE_ORGANIZATION,
  );
  let connection = rusqlite::Connection::open(old.path().join(".orchester").join("orchester.db")).expect("open");
  let task_status = if matches!(before, Before::CancelledDraft | Before::FailedDraft) {
    "draft"
  } else {
    "queued"
  };
  connection
    .execute(
      "UPDATE tasks SET status = ?1 WHERE id = ?2",
      [task_status, &run.task().to_string()],
    )
    .expect("the task's status");
  connection
    .execute(
      "UPDATE tasks SET status = 'queued' WHERE id = ?1",
      [others[1].id().to_string()],
    )
    .expect("a queued task without a run");
  drop(connection);
  let db = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(
    db.task_organization(others[0].id()).expect("draft").status(),
    WorkflowStatus::Backlog
  );
  assert_eq!(
    db.task_organization(others[1].id()).expect("queued").status(),
    WorkflowStatus::Todo
  );
  assert!(db.task_organization_history(run.task()).expect("history").is_empty());
  db.task_organization(run.task()).expect("status").status()
}

/// Migration 39 gives existing tasks the status their runs imply (user decision, 2026-10-03).
#[test]
fn organization_migration_backfills_statuses_from_runs() {
  for (before, status) in [
    (Before::Running, WorkflowStatus::InProgress),
    (Before::ReadyForReview, WorkflowStatus::InReview),
    (Before::Merged, WorkflowStatus::Done),
    (Before::CancelledDraft, WorkflowStatus::Backlog),
    (Before::CancelledQueued, WorkflowStatus::Todo),
    (Before::Paused, WorkflowStatus::InProgress),
    (Before::Interrupted, WorkflowStatus::InProgress),
    (Before::Waiting, WorkflowStatus::InProgress),
    (Before::FailedQueued, WorkflowStatus::Todo),
    (Before::FailedDraft, WorkflowStatus::Backlog),
  ] {
    assert_eq!(backfilled(before), status, "{before:?}");
  }
}
