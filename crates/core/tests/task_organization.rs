use orchester_core::{
  id::{RunId, TagId, TaskId},
  run::TaskPhase,
  task::organization::{
    BlockingTask, DependencyGate, OrganizationError, Priority, TaskFilter, TaskOrganization, WorkflowStatus,
  },
};

#[test]
fn organization_manual_statuses_and_informational_blocking() {
  let reason = Some("Waiting for a decision".to_owned());
  for status in WorkflowStatus::ALL {
    let value = TaskOrganization::new(status, Priority::None, reason.clone(), vec![], vec![]).expect("valid");
    assert!(value.unfinished(|_| false).is_empty());
    assert_eq!(value.blocked_reason(), reason.as_deref());
    assert!(status.check_change(None).is_ok());
  }
  assert!(matches!(
    WorkflowStatus::Canceled.check_change(Some(RunId::generate())),
    Err(OrganizationError::StopRunFirst(_))
  ));
  assert_eq!(
    TaskOrganization::new(WorkflowStatus::Todo, Priority::Low, Some("  ".into()), vec![], vec![]),
    Err(OrganizationError::MissingReason)
  );
}

#[test]
fn organization_filters_combine_facets_and_bulk_tags_deduplicate() {
  let first = TagId::generate();
  let second = TagId::generate();
  let value = TaskOrganization::default()
    .with_status(WorkflowStatus::InReview)
    .with_priority(Priority::Urgent)
    .add_tags(&[first, first]);
  assert_eq!(value.tags(), &[first]);
  let mut filter = TaskFilter {
    statuses: vec![WorkflowStatus::Todo, WorkflowStatus::InReview],
    priorities: vec![Priority::Urgent],
    tags: vec![first, second],
    all_tags: false,
  };
  assert!(filter.includes(&value));
  filter.all_tags = true;
  assert!(!filter.includes(&value));
  assert!(filter.includes(&value.clone().add_tags(&[second])));
  assert!(value.remove_tags(&[first]).tags().is_empty());
  assert!(Priority::Urgent < Priority::High && Priority::Low < Priority::None);
}

#[test]
fn organization_dependencies_use_execution_completion() {
  let task = TaskId::generate();
  let value = TaskOrganization::new(
    WorkflowStatus::Done,
    Priority::None,
    None,
    vec![BlockingTask {
      task,
      gate: DependencyGate::Merge,
    }],
    vec![],
  )
  .expect("valid");
  assert_eq!(
    value.unfinished(|link| link.gate.is_completed(TaskPhase::Succeeded)),
    vec![task]
  );
  assert!(
    value
      .unfinished(|link| link.gate.is_completed(TaskPhase::Merged))
      .is_empty()
  );
  assert!(DependencyGate::Review.is_completed(TaskPhase::Succeeded));
  assert!(!DependencyGate::Review.is_completed(TaskPhase::Failed));
}
