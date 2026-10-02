use std::sync::Arc;

use orchester_core::task::organization::{Priority, TaskFilter, TaskOrganization, WorkflowStatus};
use orchester_desktop::tasks::{
  browse::{self, PipelineFilter, Query, Selection, TaskSort},
  data::ProjectTasks,
  list::TaskTab,
};

/// The rows of `data` for `tab` matching `text`, `pipeline` and `organization`, in `sort` order.
fn query(
  data: &ProjectTasks,
  tab: TaskTab,
  (text, pipeline, organization): (&str, PipelineFilter, &TaskFilter),
  sort: TaskSort,
) -> Vec<orchester_desktop::tasks::list::TaskRow> {
  let query = Query {
    text,
    pipeline,
    organization,
  };
  browse::rows(data, tab, query, sort)
}

use super::stored_tasks;

#[test]
fn task_bulk_search_sort_and_pipeline_filter_combine_with_tabs() {
  let mut tasks = stored_tasks();
  let mut details = tasks[0].details().clone();
  details.brief = "Detailed scope".into();
  tasks[0].edit(details, false).expect("draft edits");
  let pipeline = tasks[1].details().pipeline;
  let data = ProjectTasks {
    tasks: Arc::new(tasks),
    ..ProjectTasks::default()
  };
  let none = TaskFilter::default();
  let rows = |tab, text, pipeline, sort| query(&data, tab, (text, PipelineFilter(pipeline), &none), sort);
  assert_eq!(rows(TaskTab::All, "orc-2", None, TaskSort::Manual).len(), 1);
  assert_eq!(rows(TaskTab::All, "detailed", None, TaskSort::Manual).len(), 1);
  assert!(rows(TaskTab::Todo, "queue", None, TaskSort::Manual).is_empty());
  assert_eq!(rows(TaskTab::Backlog, "queue", pipeline, TaskSort::Manual).len(), 1);
  assert!(rows(TaskTab::All, "detailed", pipeline, TaskSort::Manual).is_empty());
  let newest = rows(TaskTab::All, "", None, TaskSort::Newest);
  assert_eq!(&*newest[0].title, "To queue");
  let title = rows(TaskTab::All, "", None, TaskSort::Title);
  assert_eq!(&*title[0].title, "Incomplete");
}

#[test]
fn task_bulk_select_all_only_changes_visible_rows_and_preserves_hidden_selection() {
  let data = ProjectTasks {
    tasks: Arc::new(stored_tasks()),
    ..ProjectTasks::default()
  };
  let all = query(
    &data,
    TaskTab::All,
    ("", PipelineFilter(None), &TaskFilter::default()),
    TaskSort::Manual,
  );
  let visible = &all[..1];
  let mut selected = Selection::default();
  selected.toggle(all[1].id);
  selected.toggle_all(visible);
  assert_eq!(selected.0.len(), 2);
  selected.toggle_all(visible);
  assert_eq!(selected.0.iter().copied().collect::<Vec<_>>(), vec![all[1].id]);
}

/// The organization filters narrow the search's rows, and Status and Priority order them by
/// workflow and urgency.
#[test]
fn task_bulk_organization_filters_and_sorts_combine_with_the_search() {
  let tasks = stored_tasks();
  let organized = |task: &orchester_core::task::Task, status, priority| {
    let value = TaskOrganization::default().with_status(status).with_priority(priority);
    task.clone().with_organization(value)
  };
  let data = ProjectTasks {
    tasks: Arc::new(vec![
      organized(&tasks[0], WorkflowStatus::Todo, Priority::Low),
      organized(&tasks[1], WorkflowStatus::Backlog, Priority::Urgent),
    ]),
    ..ProjectTasks::default()
  };
  let all = (" ", PipelineFilter(None), &TaskFilter::default());
  let by_status = query(&data, TaskTab::All, all, TaskSort::Status);
  assert_eq!(&*by_status[0].title, "To queue", "Backlog comes before Todo");
  let by_priority = query(&data, TaskTab::All, all, TaskSort::Priority);
  assert_eq!(&*by_priority[0].title, "To queue", "Urgent comes first");
  let todo = TaskFilter {
    statuses: vec![WorkflowStatus::Todo],
    ..TaskFilter::default()
  };
  let rows = query(&data, TaskTab::All, ("", PipelineFilter(None), &todo), TaskSort::Manual);
  assert_eq!(rows.len(), 1);
  assert_eq!(&*rows[0].title, "Incomplete");
  assert!(
    query(
      &data,
      TaskTab::All,
      ("queue", PipelineFilter(None), &todo),
      TaskSort::Manual
    )
    .is_empty()
  );
}
