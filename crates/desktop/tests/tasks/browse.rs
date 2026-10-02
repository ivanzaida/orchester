use std::sync::Arc;

use orchester_desktop::tasks::{
  browse::{self, PipelineFilter, Selection, TaskSort},
  data::ProjectTasks,
  list::TaskTab,
};

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
  let rows = |tab, query, pipeline, sort| browse::rows(&data, tab, query, PipelineFilter(pipeline), sort);
  assert_eq!(rows(TaskTab::All, "orc-2", None, TaskSort::Manual).len(), 1);
  assert_eq!(rows(TaskTab::All, "detailed", None, TaskSort::Manual).len(), 1);
  assert!(rows(TaskTab::Drafts, "queue", None, TaskSort::Manual).is_empty());
  assert_eq!(rows(TaskTab::Queue, "queue", pipeline, TaskSort::Manual).len(), 1);
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
  let all = browse::rows(&data, TaskTab::All, "", PipelineFilter(None), TaskSort::Manual);
  let visible = &all[..1];
  let mut selected = Selection::default();
  selected.toggle(all[1].id);
  selected.toggle_all(visible);
  assert_eq!(selected.0.len(), 2);
  selected.toggle_all(visible);
  assert_eq!(selected.0.iter().copied().collect::<Vec<_>>(), vec![all[1].id]);
}
