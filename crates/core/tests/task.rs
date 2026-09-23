use std::{num::NonZeroU32, time::Duration};

use orchester_core::{
  id::{ResourceId, TaskId},
  money::{Currency, Money},
  name::Name,
  project::{ProjectKey, TaskKey},
  task::{LimitsError, MissingDetail, QueueError, Task, TaskDetails, TaskLimits, TaskStatus},
  time::Timestamp,
};

fn five_dollars_for_thirty_minutes() -> TaskLimits {
  TaskLimits::new(
    Money::from_micros(5_000_000, Currency::USD),
    Duration::from_secs(30 * 60),
  )
  .expect("positive budget and duration")
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
    limits: Some(five_dollars_for_thirty_minutes()),
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
  assert_eq!(task.queue(), Err(QueueError::Incomplete(expected)));
  assert_eq!(task.status(), TaskStatus::Draft);
}

#[test]
fn complete_draft_is_queued_once() {
  let mut task = task(complete());
  assert_eq!(task.queue(), Ok(()));
  assert_eq!(task.status(), TaskStatus::Queued);
  assert_eq!(task.queue(), Err(QueueError::NotDraft(TaskStatus::Queued)));
}

#[test]
fn limits_reject_empty_budget_and_duration() {
  let zero = Money::from_micros(0, Currency::USD);
  assert_eq!(
    TaskLimits::new(zero, Duration::from_secs(60)),
    Err(LimitsError::NonPositiveBudget)
  );
  let five = Money::from_micros(5_000_000, Currency::USD);
  assert_eq!(TaskLimits::new(five, Duration::ZERO), Err(LimitsError::ZeroDuration));
}
