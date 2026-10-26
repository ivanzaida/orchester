//! The Tasks list's Filter facets and its views and groups (ORC-99): each task is in one of Active, Backlog and Done,
//! a task something of waits for is in Active's Needs you whatever its status, All is grouped by workflow status.

use std::{collections::HashSet, sync::Arc};

use orchester_core::task::{
  Task,
  organization::{Priority, TaskOrganization, WorkflowStatus},
};
use orchester_workspace::tasks::{
  browse::{self, Facets, TaskSort},
  data::ProjectTasks,
  list::TaskRow,
  views::{Group, ListView, grouped},
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

#[test]
fn task_views_hold_each_task_once_and_needs_you_whatever_its_status() {
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
  assert_eq!(counts, [1, 0, 1, 2], "Active, Backlog, Done, All");
  let active = grouped(ListView::Active, &rows, (&data, &nothing), TaskSort::Manual);
  assert_eq!(active.len(), 1);
  assert_eq!(active[0].group, Some(Group::ReadyToStart), "a draft is ready to start");
  let waiting = HashSet::from([rows[1].id]);
  let counts = ListView::ALL.map(|view| view.count(&rows, &waiting));
  assert_eq!(
    counts,
    [2, 0, 0, 2],
    "a Done task that waits for the person is Active's"
  );
  let active = grouped(ListView::Active, &rows, (&data, &waiting), TaskSort::Manual);
  assert_eq!(active[0].group, Some(Group::NeedsYou), "Needs you comes first");
  assert_eq!(titles(&active[0].rows), ["To queue"]);
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
}
