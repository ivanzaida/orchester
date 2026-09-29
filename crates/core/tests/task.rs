use std::{num::NonZeroU32, time::Duration};

use orchester_core::{
  id::{ResourceId, TaskId},
  name::Name,
  project::{ProjectKey, TaskKey},
  resource::VersionNumber,
  task::{EditError, LimitsError, MissingDetail, QueueError, Task, TaskDetails, TaskLimits, TaskStatus},
  time::Timestamp,
  tokens::TokenBudget,
};

fn default_limits() -> TaskLimits {
  TaskLimits::new(
    TokenBudget::new(1_400_000).expect("above zero"),
    Duration::from_secs(30 * 60),
  )
  .expect("positive duration")
}

fn title_only() -> TaskDetails {
  TaskDetails {
    title: Name::new("Add repository switcher").expect("non-blank title"),
    brief: String::new(),
    acceptance_criteria: vec!["  ".into()],
    pipeline: None,
    limits: None,
  }
}

fn complete() -> TaskDetails {
  TaskDetails {
    brief: "Let users switch repositories from the sidebar.".into(),
    acceptance_criteria: vec!["The switcher lists connected repositories.".into()],
    pipeline: Some(ResourceId::generate()),
    limits: Some(default_limits()),
    ..title_only()
  }
}

fn task(details: TaskDetails) -> Task {
  let project = ProjectKey::new("ORC").expect("valid project key");
  let key = TaskKey::new(project, NonZeroU32::new(129).expect("non-zero"));
  Task::draft(TaskId::generate(), key, details, Timestamp::from_unix_millis(0))
}

#[test]
fn incomplete_draft_reports_every_missing_detail_and_stays_a_draft() {
  let mut task = task(title_only());
  let expected = vec![
    MissingDetail::Brief,
    MissingDetail::AcceptanceCriteria,
    MissingDetail::Pipeline,
    MissingDetail::Limits,
  ];
  assert_eq!(task.queue(VersionNumber::FIRST), Err(QueueError::Incomplete(expected)));
  assert_eq!(task.status(), TaskStatus::Draft);
}

#[test]
fn complete_draft_is_queued_once() {
  let mut task = task(complete());
  assert_eq!(task.queue(VersionNumber::FIRST), Ok(()));
  assert_eq!(task.status(), TaskStatus::Queued);
  assert_eq!(task.pipeline_version(), Some(VersionNumber::FIRST));
  assert_eq!(
    task.queue(VersionNumber::FIRST),
    Err(QueueError::NotDraft(TaskStatus::Queued))
  );
}

#[test]
fn first_run_key_survives_queue_and_reload_but_differs_for_another_task() {
  let mut first = task(complete());
  let key = first.id().first_run_key();
  assert_eq!(key.as_uuid(), first.id().as_uuid());
  first.queue(VersionNumber::FIRST).expect("queue first task");
  let reloaded = Task::from_stored(
    first.id(),
    first.key().clone(),
    first.details().clone(),
    first.status(),
    first.pipeline_version(),
    first.created_at(),
  );
  assert_eq!(key, reloaded.id().first_run_key());
  assert_ne!(key, task(complete()).id().first_run_key());
}

#[test]
fn limits_reject_an_empty_budget_and_duration() {
  assert!(TokenBudget::new(0).is_err());
  let budget = TokenBudget::new(1_400_000).expect("above zero");
  assert_eq!(TaskLimits::new(budget, Duration::ZERO), Err(LimitsError::ZeroDuration));
}

#[test]
fn edit_replaces_a_drafts_details_and_keeps_its_identity() {
  let mut task = task(title_only());
  let (id, key) = (task.id(), task.key().clone());
  let details = complete();
  assert_eq!(task.edit(details.clone(), false), Ok(()));
  assert_eq!(task.details(), &details);
  assert_eq!((task.id(), task.key()), (id, &key));
  assert_eq!(task.status(), TaskStatus::Draft);
}

#[test]
fn edit_is_refused_once_the_task_has_a_run_or_left_the_drafts() {
  let mut started = task(title_only());
  assert_eq!(started.edit(complete(), true), Err(EditError::HasRun));
  assert_eq!(started.details(), &title_only());

  let details = complete();
  let mut queued = task(details.clone());
  queued.queue(VersionNumber::FIRST).expect("a complete draft queues");
  assert_eq!(
    queued.edit(title_only(), false),
    Err(EditError::NotDraft(TaskStatus::Queued))
  );
  assert_eq!(queued.details(), &details);
}
