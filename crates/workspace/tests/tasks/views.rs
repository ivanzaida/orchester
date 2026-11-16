//! The Tasks list's Filter facets and its views and groups (ORC-99): each task is in one of Active, Backlog and Done,
//! a task something of waits for is in Active's Needs you whatever its status, All is grouped by workflow status.

use std::{collections::HashSet, sync::Arc};

use orchester_core::{
  id::TaskId,
  task::{
    Task,
    organization::{Priority, TaskOrganization, WorkflowStatus},
  },
};
use orchester_workspace::tasks::{
  browse::{self, Facets, TaskSort},
  data::ProjectTasks,
  list::TaskRow,
  views::{Group, ListView, StatusCounts, grouped},
};

use super::stored_tasks;

fn organized(task: &Task, status: WorkflowStatus, priority: Priority) -> Task {
  let value = TaskOrganization::default().with_status(status).with_priority(priority);
  task.clone().with_organization(value)
}

fn titles(rows: &[TaskRow]) -> Vec<&str> {
  rows.iter().map(|row| &*row.title).collect()
}

#[test]
fn task_views_facets_keep_a_task_with_any_chosen_value_of_each_facet() {
  let tasks = stored_tasks();
  let pipeline = tasks[1].details().pipeline;
  assert!(pipeline.is_some() && tasks[0].details().pipeline.is_none());
  let data = ProjectTasks {
    tasks: Arc::new(vec![
      organized(&tasks[0], WorkflowStatus::Todo, Priority::Low),
      organized(&tasks[1], WorkflowStatus::Todo, Priority::Urgent),
    ]),
    ..ProjectTasks::default()
  };
  let rows = |facets: &Facets, text: &str| browse::faceted(&data, (text, ", "), facets, TaskSort::Manual);
  let none = Facets::default();
  assert_eq!(
    titles(&rows(&none, "")),
    ["Incomplete", "To queue"],
    "no facet keeps every task"
  );
  let without = Facets {
    pipelines: vec![None],
    ..Facets::default()
  };
  assert_eq!(
    titles(&rows(&without, "")),
    ["Incomplete"],
    "None is a task without a pipeline"
  );
  let either = Facets {
    pipelines: vec![None, pipeline],
    ..Facets::default()
  };
  assert_eq!(titles(&rows(&either, "")).len(), 2, "any chosen value of a facet");
  let urgent = Facets {
    pipelines: vec![None, pipeline],
    priorities: vec![Priority::Urgent],
    ..Facets::default()
  };
  assert_eq!(titles(&rows(&urgent, "")), ["To queue"], "every facet that has a value");
  assert_eq!(urgent.count(), 3);
  assert!(rows(&urgent, "incomplete").is_empty(), "the search narrows them too");
}

/// The row of the task titled *To queue*, the second of [`stored_tasks`].
fn to_queue(rows: &[TaskRow]) -> TaskId {
  rows.iter().find(|row| &*row.title == "To queue").expect("the task").id
}

#[test]
fn task_views_hold_each_task_once_and_needs_you_whatever_its_open_status() {
  let tasks = stored_tasks();
  let data = ProjectTasks {
    tasks: Arc::new(vec![
      organized(&tasks[0], WorkflowStatus::Todo, Priority::Low),
      organized(&tasks[1], WorkflowStatus::Done, Priority::High),
    ]),
    ..ProjectTasks::default()
  };
  let rows = browse::faceted(&data, ("", ", "), &Facets::default(), TaskSort::Manual);
  let nothing = HashSet::new();
  let counts = ListView::ALL.map(|view| view.count(&rows, &nothing));
  assert_eq!(counts, [0, 1, 0, 1, 2], "Needs you, Active, Backlog, Done, All");
  let active = grouped(ListView::Active, &rows, (&data, &nothing), TaskSort::Manual);
  assert_eq!(active.len(), 1);
  assert_eq!(active[0].group, Some(Group::ReadyToStart), "a draft is ready to start");
  let waiting = HashSet::from([to_queue(&rows)]);
  let counts = ListView::ALL.map(|view| view.count(&rows, &waiting));
  assert_eq!(
    counts,
    [0, 1, 0, 1, 2],
    "a Done task is Done's only, even while something of it waits (ORC-150)"
  );
  let all = grouped(ListView::All, &rows, (&data, &waiting), TaskSort::Manual);
  let groups: Vec<Option<Group>> = all.iter().map(|part| part.group).collect();
  assert_eq!(
    groups,
    [
      Some(Group::Workflow(WorkflowStatus::Todo)),
      Some(Group::Workflow(WorkflowStatus::Done))
    ]
  );
  let done = grouped(ListView::Done, &rows, (&data, &nothing), TaskSort::Manual);
  assert_eq!(done.len(), 1);
  assert_eq!(done[0].group, None, "Done is flat");

  let data = ProjectTasks {
    tasks: Arc::new(vec![
      organized(&tasks[0], WorkflowStatus::Todo, Priority::Low),
      organized(&tasks[1], WorkflowStatus::Backlog, Priority::High),
    ]),
    ..ProjectTasks::default()
  };
  let rows = browse::faceted(&data, ("", ", "), &Facets::default(), TaskSort::Manual);
  let counts = ListView::ALL.map(|view| view.count(&rows, &nothing));
  assert_eq!(
    counts,
    [0, 1, 1, 0, 2],
    "a Backlog task that waits for no one is Backlog's"
  );
  let waiting = HashSet::from([to_queue(&rows)]);
  let counts = ListView::ALL.map(|view| view.count(&rows, &waiting));
  assert_eq!(
    counts,
    [1, 2, 0, 0, 2],
    "a Backlog task that waits for the person is Needs you's and Active's"
  );
  let active = grouped(ListView::Active, &rows, (&data, &waiting), TaskSort::Manual);
  assert_eq!(active[0].group, Some(Group::NeedsYou), "Needs you comes first");
  assert_eq!(titles(&active[0].rows), ["To queue"]);
  let needs_you = grouped(ListView::NeedsYou, &rows, (&data, &waiting), TaskSort::Manual);
  assert_eq!(needs_you.len(), 1);
  assert_eq!(needs_you[0].group, None, "Needs you is flat");
  assert_eq!(titles(&needs_you[0].rows), ["To queue"], "only what waits");
}

/// A team server is asked for each view's statuses and counts by status (ORC-716): every task a view keeps is in one of
/// its statuses, and the views counted by status, with what waits placed by its status, are the views counted by row.
#[test]
fn task_views_counted_by_status_agree_with_the_rows_each_view_keeps() {
  let data = ProjectTasks {
    tasks: Arc::new(stored_tasks()),
    ..ProjectTasks::default()
  };
  let row = browse::faceted(&data, ("", ", "), &Facets::default(), TaskSort::Manual).remove(0);
  let rows: Vec<TaskRow> = WorkflowStatus::ALL
    .into_iter()
    .flat_map(|status| [status, status])
    .map(|status| TaskRow {
      id: TaskId::generate(),
      organization: TaskOrganization::default().with_status(status),
      ..row.clone()
    })
    .collect();
  let waiting: HashSet<TaskId> = rows.iter().step_by(2).map(|row| row.id).collect();
  let counted = StatusCounts::of(rows.iter().map(|row| row.organization.status()));
  let waits = StatusCounts::of(
    rows
      .iter()
      .filter(|row| waiting.contains(&row.id))
      .map(|row| row.organization.status()),
  );
  for view in ListView::ALL {
    assert_eq!(
      view.count_by_status(&counted, &waits),
      view.count(&rows, &waiting),
      "{view:?}"
    );
    let asked = view.statuses();
    for kept in rows.iter().filter(|row| view.keeps(row, &waiting)) {
      assert!(
        asked.contains(&kept.organization.status()),
        "{view:?} asks for {asked:?}"
      );
    }
  }
  assert!(
    ListView::Active.statuses().contains(&WorkflowStatus::Backlog),
    "a Backlog task that waits is Active's"
  );
  assert_eq!(
    ListView::Backlog.count_by_status(&counted, &waits),
    1,
    "the Backlog task that waits is not"
  );
}
