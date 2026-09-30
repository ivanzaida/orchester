mod support;

use orchester_core::{
  id::IdempotencyKey,
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::BranchName,
  store::{MergeStore, RunStore, TaskStore},
  task::organization::{
    BlockingTask, ChangeActor, DependencyGate, OrganizationError, Priority, TaskFilter, TaskOrganization, TaskSort,
    WorkflowStatus,
  },
};
use orchester_store::StoreError;
use support::{at, name, started_run};

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
  db.rename_project_tag(source.id(), &name("DESKTOP"))
    .expect("rename itself ignoring case");
  db.merge_project_tags(source.id(), target.id(), at(32)).expect("merge");
  assert_eq!(db.task_organization(ids[0]).expect("metadata").tags(), &[target.id()]);
  let filter = TaskFilter {
    tags: vec![target.id()],
    priorities: vec![Priority::High],
    all_tags: true,
    ..TaskFilter::default()
  };
  assert_eq!(
    db.organized_task_ids(&filter, TaskSort::Priority)
      .expect("filter")
      .len(),
    2
  );
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
  assert!(!db.task_organization_history(ids[0]).expect("kept history").is_empty());
  let recreated = db.create_project_tag(name("UX")).expect("recreate");
  assert_ne!(recreated.id(), target.id());
  assert_eq!(db.tag_task_count(recreated.id()).expect("count"), 0);
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
