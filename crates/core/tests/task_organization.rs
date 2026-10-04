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
fn organization_duplicate_links_keep_one_gate_per_task() {
  let task = TaskId::generate();
  let value = TaskOrganization::new(
    WorkflowStatus::Todo,
    Priority::None,
    None,
    vec![
      BlockingTask {
        task,
        gate: DependencyGate::Review,
      },
      BlockingTask {
        task,
        gate: DependencyGate::Merge,
      },
    ],
    vec![],
  )
  .expect("organization");
  assert_eq!(
    value.dependencies(),
    &[BlockingTask {
      task,
      gate: DependencyGate::Merge
    }]
  );
  let restored: TaskOrganization =
    serde_json::from_value(serde_json::to_value(&value).expect("encode")).expect("decode");
  assert_eq!(value, restored);
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

#[test]
fn organization_sorting_uses_workflow_priority_then_recent_update() {
  use std::cmp::Ordering;

  use orchester_core::{task::organization::TaskSort, time::Timestamp};
  let first = TaskId::generate();
  let second = TaskId::generate();
  let old = Timestamp::from_unix_millis(1);
  let recent = Timestamp::from_unix_millis(2);
  let backlog = TaskOrganization::default().with_priority(Priority::Urgent);
  let todo = TaskOrganization::default().with_status(WorkflowStatus::Todo);
  assert_eq!(
    TaskSort::Status.compare((&backlog, old, first), (&todo, recent, second)),
    Ordering::Less
  );
  assert_eq!(
    TaskSort::Priority.compare((&backlog, old, first), (&todo, recent, second)),
    Ordering::Less
  );
  assert_eq!(
    TaskSort::Priority.compare((&backlog, old, first), (&backlog, recent, second)),
    Ordering::Greater
  );
}

#[test]
fn organization_rejects_indirect_dependency_cycles() {
  use std::collections::HashMap;
  let first = TaskId::generate();
  let second = TaskId::generate();
  let linked = |task| {
    TaskOrganization::new(
      WorkflowStatus::Todo,
      Priority::None,
      None,
      vec![BlockingTask {
        task,
        gate: DependencyGate::Merge,
      }],
      vec![],
    )
    .expect("organization")
  };
  let graph = HashMap::from([(second, linked(first))]);
  assert_eq!(
    linked(second).check_dependencies(first, &graph),
    Err(OrganizationError::DependencyCycle)
  );
  TaskOrganization::default()
    .check_dependencies(first, &graph)
    .expect("removing link resolves cycle");
}

/// A form's save keeps what changed meanwhile: only the fields the person changed from what the
/// form loaded replace the stored ones, and tags and links merge by what was added and removed.
#[test]
fn organization_rebase_keeps_changes_made_meanwhile() {
  let (desktop, ux, bug) = (TagId::generate(), TagId::generate(), TagId::generate());
  let (first, second) = (TaskId::generate(), TaskId::generate());
  let link = |task| BlockingTask {
    task,
    gate: DependencyGate::Merge,
  };
  let base = TaskOrganization::new(
    WorkflowStatus::InProgress,
    Priority::Low,
    None,
    vec![link(first)],
    vec![desktop],
  )
  .expect("loaded");
  // Meanwhile Orchester moved the task to In review and a bulk change added UX.
  let current = base.clone().with_status(WorkflowStatus::InReview).add_tags(&[ux]);
  // Unchanged form: what is stored stays.
  assert_eq!(base.rebase(&base, &current), current);
  // The person raised the priority, swapped Desktop for Bug and the first link for the second.
  let edited = TaskOrganization::new(
    WorkflowStatus::InProgress,
    Priority::High,
    None,
    vec![link(second)],
    vec![bug],
  )
  .expect("edited");
  let saved = edited.rebase(&base, &current);
  assert_eq!(saved.status(), WorkflowStatus::InReview);
  assert_eq!(saved.priority(), Priority::High);
  let mut tags = vec![ux, bug];
  tags.sort();
  assert_eq!(saved.tags(), tags.as_slice());
  assert_eq!(saved.dependencies(), [link(second)]);
}

/// The hold and the refusal a blocked task causes cross the runner's IPC as JSON.
#[test]
fn organization_blocked_hold_and_refusal_round_trip() {
  use orchester_core::{run::RunAgainBlock, task_group::GroupHold};
  let hold = GroupHold::Blocked {
    by: vec![TaskId::generate(), TaskId::generate()],
  };
  let json = serde_json::to_string(&hold).expect("hold");
  assert_eq!(serde_json::from_str::<GroupHold>(&json).expect("hold back"), hold);
  let block = RunAgainBlock::TaskBlocked;
  let json = serde_json::to_string(&block).expect("block");
  assert_eq!(serde_json::from_str::<RunAgainBlock>(&json).expect("block back"), block);
}
