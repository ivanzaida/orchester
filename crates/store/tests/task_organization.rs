mod support;

use orchester_core::{
  id::IdempotencyKey,
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::BranchName,
  store::{MergeStore, RunStore, TaskStore},
  task::organization::{
    BlockingTask, ChangeActor, DependencyGate, OrganizationError, Priority, TaskFilter, TaskOrganization,
    WorkflowStatus,
  },
};
use orchester_store::{ProjectDb, StoreError};
use support::{at, copied_at_version, name, started_run};

#[test]
fn organization_defaults_transitions_and_cancel_rule() {
  let (_repo, mut db, mut run, _) = started_run();
  let task = run.task();
  let value = db.task_organization(task).expect("metadata");
  assert_eq!(value.status(), WorkflowStatus::InProgress);
  assert_eq!(value.priority(), Priority::None);
  assert_eq!(
    db.task_organization_history(task).expect("history")[0].actor,
    ChangeActor::Orchester
  );
  let canceled = value.clone().with_status(WorkflowStatus::Canceled);
  assert!(matches!(
    db.set_task_organization(task, &canceled, ChangeActor::Person, at(9)),
    Err(StoreError::Organization(OrganizationError::StopRunFirst(_)))
  ));
  assert_eq!(db.task_organization(task).expect("unchanged"), value);
  let events = run.cancel(at(10)).expect("cancel run");
  db.append_events(&run, &events).expect("persist cancellation");
  db.set_task_organization(task, &canceled, ChangeActor::Person, at(11))
    .expect("set canceled after stop");
  let history = db.task_organization_history(task).expect("history");
  assert_eq!(history.last().expect("change").at, at(11));
  assert_eq!(
    history.last().expect("change").from.status(),
    WorkflowStatus::InProgress
  );
  assert_eq!(history.last().expect("change").to.status(), WorkflowStatus::Canceled);
  assert_eq!(db.run(run.id()).expect("read run").expect("run").branch(), run.branch());
}

#[test]
fn organization_blocked_links_resolve_without_clearing_manual_reason() {
  let (_repo, mut db, run, _) = started_run();
  let details = db.task(run.task()).expect("task").expect("exists").details().clone();
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(20))
    .expect("new task")[0]
    .id();
  let reason = Some("Waiting for the integration".to_owned());
  let informational =
    TaskOrganization::new(WorkflowStatus::Todo, Priority::High, reason.clone(), vec![], vec![]).expect("metadata");
  db.set_task_organization(task, &informational, ChangeActor::Person, at(21))
    .expect("flag");
  db.check_task_readiness(task).expect("reason-only is informational");
  let linked = TaskOrganization::new(
    WorkflowStatus::Todo,
    Priority::High,
    reason.clone(),
    vec![BlockingTask {
      task: run.task(),
      gate: DependencyGate::Merge,
    }],
    vec![],
  )
  .expect("linked");
  db.set_task_organization(task, &linked, ChangeActor::Person, at(22))
    .expect("link");
  assert!(db.check_task_readiness(task).is_err());
  db.set_task_organization(
    run.task(),
    &TaskOrganization::default().with_status(WorkflowStatus::Done),
    ChangeActor::Person,
    at(23),
  )
  .expect("manual Done");
  assert!(
    db.check_task_readiness(task).is_err(),
    "manual Done does not satisfy merge"
  );
  db.save_run_merge(&RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("branch"),
    revision: Revision::new(&"b".repeat(40)).expect("revision"),
    kind: MergeKind::FastForward,
    merged_at: at(24),
  })
  .expect("merge recorded");
  db.check_task_readiness(task).expect("resolved");
  assert_eq!(
    db.task_organization(task).expect("metadata").blocked_reason(),
    reason.as_deref()
  );
  let self_link = TaskOrganization::new(
    WorkflowStatus::Todo,
    Priority::None,
    None,
    vec![BlockingTask {
      task,
      gate: DependencyGate::Review,
    }],
    vec![],
  )
  .expect("link");
  assert!(matches!(
    db.set_task_organization(task, &self_link, ChangeActor::Person, at(25)),
    Err(StoreError::Organization(OrganizationError::DependencyCycle))
  ));
}

#[test]
fn organization_tags_uniqueness_merge_delete_and_bulk_filters_sorting() {
  let (_repo, mut db, run, _) = started_run();
  let details = db.task(run.task()).expect("read").expect("task").details().clone();
  let tasks = db
    .create_tasks(IdempotencyKey::generate(), vec![details.clone(), details], at(30))
    .expect("tasks");
  let ids: Vec<_> = tasks.iter().map(|task| task.id()).collect();
  let source = db.create_project_tag(name("Desktop")).expect("tag");
  assert!(matches!(
    db.create_project_tag(name(" desktop ")),
    Err(StoreError::Organization(OrganizationError::DuplicateTag))
  ));
  let target = db.create_project_tag(name("UX")).expect("tag");
  db.update_task_organizations(&ids, ChangeActor::Person, at(31), |value| {
    value
      .with_tags(vec![source.id(), target.id()])
      .with_priority(Priority::High)
  })
  .expect("bulk");
  // Each tag on both new tasks, counted once per task.
  let counts = db.tag_task_counts().expect("counts");
  assert_eq!(counts.get(&source.id()), Some(&2));
  assert_eq!(counts.get(&target.id()), Some(&2));
  db.rename_project_tag(source.id(), &name("DESKTOP"))
    .expect("rename itself ignoring case");
  db.merge_project_tags(source.id(), target.id(), at(32)).expect("merge");
  assert_eq!(db.task_organization(ids[0]).expect("metadata").tags(), &[target.id()]);
  assert_eq!(db.tag_task_counts().expect("counts"), [(target.id(), 2)].into());
  let filter = TaskFilter {
    tags: vec![target.id()],
    priorities: vec![Priority::High],
    all_tags: true,
    ..TaskFilter::default()
  };
  let matching = ids
    .iter()
    .filter(|id| filter.includes(&db.task_organization(**id).expect("metadata")))
    .count();
  assert_eq!(matching, 2);
  assert_eq!(
    db.tasks()
      .expect("scheduler order")
      .iter()
      .map(|task| task.id())
      .collect::<Vec<_>>(),
    vec![run.task(), ids[0], ids[1]]
  );
  db.delete_project_tag(target.id(), at(33)).expect("delete assignments");
  assert!(db.task_organization(ids[0]).expect("metadata").tags().is_empty());
  let history = db.task_organization_history(ids[0]).expect("kept history");
  assert_eq!(
    history[0]
      .to_tag_names
      .iter()
      .map(|name| name.as_str())
      .collect::<Vec<_>>(),
    vec!["Desktop", "UX"]
  );
  let deletion = history.last().expect("deletion");
  assert_eq!(deletion.actor, ChangeActor::Person);
  assert_eq!(deletion.at, at(33));
  assert_eq!(deletion.from_tag_names, vec![name("UX")]);
  assert!(deletion.to_tag_names.is_empty());
  let recreated = db.create_project_tag(name("UX")).expect("recreate");
  assert_ne!(recreated.id(), target.id());
  assert!(!db.tag_task_counts().expect("counts").contains_key(&recreated.id()));
  assert!(
    db.set_task_organization(
      ids[0],
      &TaskOrganization::default().with_tags(vec![target.id()]),
      ChangeActor::Person,
      at(34)
    )
    .is_err()
  );
}

#[test]
fn organization_ready_for_review_and_merge_record_automatic_history() {
  let (_repo, mut db, mut run, execution) = started_run();
  let events = run.start_step(execution, at(10)).expect("step starts");
  db.append_events(&run, &events).expect("persist start");
  let events = run.complete_step(execution, at(11)).expect("step completes");
  db.append_events(&run, &events).expect("persist completion");
  assert_eq!(
    db.task_organization(run.task()).expect("metadata").status(),
    WorkflowStatus::InReview
  );
  db.save_run_merge(&RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("branch"),
    revision: Revision::new(&"b".repeat(40)).expect("revision"),
    kind: MergeKind::FastForward,
    merged_at: at(12),
  })
  .expect("merge");
  let history = db.task_organization_history(run.task()).expect("history");
  assert_eq!(history.len(), 3);
  assert!(history.iter().all(|change| change.actor == ChangeActor::Orchester));
  assert_eq!(history[1].at, at(11));
  assert_eq!(history[2].from.status(), WorkflowStatus::InReview);
  assert_eq!(history[2].to.status(), WorkflowStatus::Done);
}

#[test]
fn organization_invalid_form_save_and_bulk_cancel_leave_content_unchanged() {
  let (_repo, mut db, run, _) = started_run();
  let task = db.task(run.task()).expect("read").expect("task");
  let mut details = task.details().clone();
  details.title = name("Changed title");
  let canceled = task.organization().clone().with_status(WorkflowStatus::Canceled);
  assert!(db.update_organized_task(task.id(), details, &canceled, at(30)).is_err());
  assert_eq!(
    db.task(task.id()).expect("read").expect("task").details(),
    task.details()
  );
  let other = db
    .create_tasks(IdempotencyKey::generate(), vec![task.details().clone()], at(31))
    .expect("task")[0]
    .id();
  assert!(
    db.update_task_organizations(&[other, task.id()], ChangeActor::Person, at(32), |value| value
      .with_status(WorkflowStatus::Canceled))
      .is_err()
  );
  assert_eq!(
    db.task_organization(other).expect("metadata").status(),
    WorkflowStatus::Backlog
  );
  assert!(db.task_organization_history(other).expect("history").is_empty());
}

#[derive(Clone, Copy)]
enum Restart {
  FollowUp,
  Recovery,
  Resume,
  Continue,
  Reconnect,
}

#[test]
fn organization_restarts_validate_readiness_atomically_and_record_history() {
  use orchester_core::run::RunEventKind;
  for (restart, status) in [
    (Restart::FollowUp, WorkflowStatus::InReview),
    (Restart::Recovery, WorkflowStatus::Done),
    (Restart::Resume, WorkflowStatus::Done),
    (Restart::Continue, WorkflowStatus::Done),
    (Restart::Reconnect, WorkflowStatus::Done),
  ] {
    let (_repo, mut db, mut run, execution) = started_run();
    prepare_restart(&mut db, &mut run, execution, restart);
    let details = db.task(run.task()).expect("read").expect("task").details().clone();
    let blocker = db
      .create_tasks(IdempotencyKey::generate(), vec![details], at(20))
      .expect("blocker")[0]
      .id();
    let linked = TaskOrganization::new(
      status,
      Priority::High,
      Some("Needs integration".into()),
      vec![BlockingTask {
        task: blocker,
        gate: DependencyGate::Merge,
      }],
      vec![],
    )
    .expect("organization");
    db.set_task_organization(run.task(), &linked, ChangeActor::Person, at(21))
      .expect("link");
    let stored = run.clone();
    let before_events = db.events(run.id()).expect("events");
    let before_history = db.task_organization_history(run.task()).expect("history");
    let events = match restart {
      Restart::FollowUp => run.follow_up(&run.follow_up_step(None).expect("step"), at(22)),
      Restart::Recovery => run.recover(&run.recovery_step(None).expect("step"), at(22)),
      Restart::Resume => run.resume(at(22)),
      Restart::Continue => run.continue_run(at(22)),
      Restart::Reconnect => run.continue_after_reconnect(at(22)),
    }
    .expect("restart");
    assert!(matches!(
      events[0].kind(),
      RunEventKind::Reopened | RunEventKind::RecoveryRequested | RunEventKind::Resumed | RunEventKind::Continued { .. }
    ));
    assert!(
      db.append_events(&run, &events).is_err(),
      "unfinished link rejects restart"
    );
    assert_eq!(db.run(run.id()).expect("read").expect("run"), stored);
    assert_eq!(db.events(run.id()).expect("events"), before_events);
    assert_eq!(
      db.task_organization_history(run.task()).expect("history"),
      before_history
    );
    assert_eq!(db.task_organization(run.task()).expect("organization"), linked);
    let informational = TaskOrganization::new(status, Priority::High, Some("Needs integration".into()), vec![], vec![])
      .expect("organization");
    db.set_task_organization(run.task(), &informational, ChangeActor::Person, at(23))
      .expect("unlink");
    db.append_events(&run, &events).expect("reason-only permits restart");
    assert_eq!(
      db.task_organization(run.task()).expect("organization"),
      informational.with_status(WorkflowStatus::InProgress)
    );
    let history = db.task_organization_history(run.task()).expect("history");
    let change = history.last().expect("automatic change");
    assert_eq!(change.actor, ChangeActor::Orchester);
    assert_eq!(change.at, at(22));
    assert_eq!(change.from.status(), status);
    assert_eq!(change.to.status(), WorkflowStatus::InProgress);
  }
}

fn prepare_restart(
  db: &mut orchester_store::ProjectDb,
  run: &mut orchester_core::run::Run,
  execution: orchester_core::id::StepExecutionId,
  restart: Restart,
) {
  use orchester_core::run::StepFailure;
  if matches!(restart, Restart::Resume) {
    let events = run.pause(at(10)).expect("pause");
    db.append_events(run, &events).expect("pause persisted");
    return;
  }
  if matches!(restart, Restart::Continue | Restart::Reconnect) {
    let events = run
      .interrupt(orchester_core::run::InterruptCause::RunnerLost, at(10))
      .expect("interrupt");
    db.append_events(run, &events).expect("interrupt persisted");
    return;
  }
  let events = run.start_step(execution, at(10)).expect("start step");
  db.append_events(run, &events).expect("persist start");
  let events = match restart {
    Restart::FollowUp => run.complete_step(execution, at(11)),
    Restart::Recovery => run.fail_step(execution, StepFailure::ReportedFailed, at(11)),
    Restart::Resume | Restart::Continue | Restart::Reconnect => unreachable!("continuations returned above"),
  }
  .expect("finish step");
  db.append_events(run, &events).expect("persist finish");
}

#[test]
fn organization_failed_follow_up_restores_review_and_retains_transition_history() {
  use orchester_core::run::StepFailure;
  let (_repo, mut db, mut run, execution) = started_run();
  prepare_restart(&mut db, &mut run, execution, Restart::FollowUp);
  let events = run
    .follow_up(&run.follow_up_step(None).expect("step"), at(30))
    .expect("follow up");
  db.append_events(&run, &events).expect("restart");
  let follow_up = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.start_step(follow_up, at(31)).expect("start step");
  db.append_events(&run, &events).expect("start");
  let events = run
    .fail_step(follow_up, StepFailure::ReportedFailed, at(32))
    .expect("failed follow up");
  db.append_events(&run, &events).expect("persist failure");
  let history = db.task_organization_history(run.task()).expect("history");
  let change = history.last().expect("review change");
  assert_eq!(change.from.status(), WorkflowStatus::InProgress);
  assert_eq!(change.to.status(), WorkflowStatus::InReview);
  assert_eq!(change.actor, ChangeActor::Orchester);
  assert_eq!(change.at, at(32));
}

/// The schema version before tasks had an organization: master's last migration, stuck runs.
const BEFORE_ORGANIZATION: u32 = 36;

/// A project database of schema 36 opens with its tasks and runs, each task in Backlog with no
/// priority, tags or links, and takes organization changes.
#[test]
fn organization_migration_keeps_tasks_and_runs_of_schema_36() {
  let (repo, db, run, _) = started_run();
  drop(db);
  let current = repo.path().join(".orchester").join("orchester.db");
  let old = copied_at_version(&current, BEFORE_ORGANIZATION);
  let mut db = ProjectDb::open(old.path()).expect("migrate existing project");
  let task = run.task();
  assert_eq!(db.run(run.id()).expect("read").expect("run"), run);
  assert_eq!(
    db.task_organization(task).expect("organization"),
    TaskOrganization::default()
  );
  let value = TaskOrganization::default().with_priority(Priority::High);
  db.set_task_organization(task, &value, ChangeActor::Person, at(20))
    .expect("organize a migrated task");
  assert_eq!(db.task_organization(task).expect("organization"), value);
}

/// The store reads a blocking task as finished exactly when the domain rule
/// (`DependencyGate::is_completed` of its task phase) does, for each gate and each state of the
/// blocking task's latest run: none, running, ready for review, merged and cancelled.
#[test]
fn organization_unfinished_follows_the_dependency_gate_rule() {
  use orchester_core::run::TaskPhase;
  #[derive(Clone, Copy, Debug)]
  enum Latest {
    NoRun,
    Running,
    Succeeded,
    Merged,
    Cancelled,
  }
  for latest in [
    Latest::NoRun,
    Latest::Running,
    Latest::Succeeded,
    Latest::Merged,
    Latest::Cancelled,
  ] {
    for gate in [DependencyGate::Review, DependencyGate::Merge] {
      let (_repo, mut db, mut run, execution) = started_run();
      let template = db.task(run.task()).expect("read").expect("task").details().clone();
      let drafts = db
        .create_tasks(IdempotencyKey::generate(), vec![template.clone(), template], at(30))
        .expect("tasks");
      let blocker = match latest {
        Latest::NoRun => drafts[1].id(),
        _ => run.task(),
      };
      if matches!(latest, Latest::Succeeded | Latest::Merged) {
        let events = run.start_step(execution, at(31)).expect("start step");
        db.append_events(&run, &events).expect("stored");
        let events = run.complete_step(execution, at(32)).expect("complete step");
        db.append_events(&run, &events).expect("stored");
      }
      if matches!(latest, Latest::Merged) {
        db.save_run_merge(&RunMerge {
          run: run.id(),
          into: BranchName::new("main").expect("branch"),
          revision: Revision::new(&"b".repeat(40)).expect("revision"),
          kind: MergeKind::FastForward,
          merged_at: at(33),
        })
        .expect("merge");
      }
      if matches!(latest, Latest::Cancelled) {
        let events = run.cancel(at(31)).expect("cancel");
        db.append_events(&run, &events).expect("stored");
      }
      let linked = TaskOrganization::new(
        WorkflowStatus::Todo,
        Priority::None,
        Some("Waits.".into()),
        vec![BlockingTask { task: blocker, gate }],
        vec![],
      )
      .expect("linked");
      db.set_task_organization(drafts[0].id(), &linked, ChangeActor::Person, at(34))
        .expect("link");
      let blocker_task = db.task(blocker).expect("read").expect("blocker");
      let last = db
        .runs_of_task(blocker)
        .expect("runs")
        .pop()
        .map(|run| run.expect("readable"));
      let merge = last.as_ref().and_then(|run| db.run_merge(run.id()).expect("merge"));
      let phase = TaskPhase::of(&blocker_task, last.as_ref(), merge.as_ref());
      let unfinished = db.unfinished_blocking_tasks(drafts[0].id()).expect("unfinished");
      assert_eq!(
        unfinished.is_empty(),
        gate.is_completed(phase),
        "{latest:?} under {gate:?} ({phase:?})"
      );
    }
  }
}
