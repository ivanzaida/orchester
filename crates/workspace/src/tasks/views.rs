//! The Tasks list's views and groups (ORC-99). Needs you holds only the tasks something of waits for the person
//! ([`ProjectTasks::waiting_tasks`]), so it names exactly the tasks every *Needs you* counts (ORC-105). Active holds
//! the Todo, In progress and In review tasks and every other task that waits but a closed one; Done holds every Done
//! and Canceled task, whatever its latest run says (ORC-150), Backlog the rest, and All every task. Each task is in
//! one of Active, Backlog and Done.
//!
//! Active is grouped by what happens next: Needs you, Running, Queued, Ready to start, and the two groups the design
//! has no row for: Stopped (a run that failed, paused, was interrupted, canceled, or cannot be read, and waits for no
//! one) and Finished (every step passed, or merged, while the merge queue or nothing more needs the person). All is
//! grouped by workflow status. Needs you, Backlog and Done are flat, Done newest first.

use std::collections::HashSet;

use orchester_core::{id::TaskId, run::TaskPhase, task::organization::WorkflowStatus};

use super::{browse::TaskSort, data::ProjectTasks, list::TaskRow};

/// A view of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListView {
  NeedsYou,
  Active,
  Backlog,
  Done,
  All,
}

impl ListView {
  pub const ALL: [Self; 5] = [Self::NeedsYou, Self::Active, Self::Backlog, Self::Done, Self::All];

  /// Whether the view holds `row`, `waiting` being the tasks something of waits for the person. A closed task is in
  /// Done only, even while `waiting` names it.
  pub fn keeps(self, row: &TaskRow, waiting: &HashSet<TaskId>) -> bool {
    let status = row.organization.status();
    let waits = !status.is_closed() && waiting.contains(&row.id);
    match self {
      Self::NeedsYou => waits,
      Self::Active => {
        waits
          || matches!(
            status,
            WorkflowStatus::Todo | WorkflowStatus::InProgress | WorkflowStatus::InReview
          )
      }
      Self::Backlog => !waits && status == WorkflowStatus::Backlog,
      Self::Done => status.is_closed(),
      Self::All => true,
    }
  }

  /// How many of `rows` the view holds.
  pub fn count(self, rows: &[TaskRow], waiting: &HashSet<TaskId>) -> usize {
    rows.iter().filter(|row| self.keeps(row, waiting)).count()
  }
}

/// A group of a view's rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
  NeedsYou,
  Running,
  Queued,
  ReadyToStart,
  Stopped,
  Finished,
  /// All's groups, one per workflow status.
  Workflow(WorkflowStatus),
}

impl Group {
  /// Active's groups, in their order.
  pub const ACTIVE: [Self; 6] = [
    Self::NeedsYou,
    Self::Running,
    Self::Queued,
    Self::ReadyToStart,
    Self::Stopped,
    Self::Finished,
  ];

  /// The group of Active `row` is in: what happens next to it.
  pub fn next(row: &TaskRow, waiting: &HashSet<TaskId>) -> Self {
    if waiting.contains(&row.id) {
      return Self::NeedsYou;
    }
    match row.phase {
      // A run that waits for an answer this window already sent works on.
      Some(TaskPhase::Running | TaskPhase::Waiting) => Self::Running,
      Some(TaskPhase::Queued) => Self::Queued,
      Some(TaskPhase::Draft) => Self::ReadyToStart,
      Some(TaskPhase::Succeeded | TaskPhase::Merged) => Self::Finished,
      Some(
        TaskPhase::Stuck
        | TaskPhase::Paused
        | TaskPhase::StoppedAtLimit
        | TaskPhase::Interrupted
        | TaskPhase::Failed
        | TaskPhase::Cancelled,
      )
      | None => Self::Stopped,
    }
  }

  /// The group's name for tools and element ids.
  pub const fn name(self) -> &'static str {
    match self {
      Self::NeedsYou => "needs_you",
      Self::Running => "running",
      Self::Queued => "queued",
      Self::ReadyToStart => "ready",
      Self::Stopped => "stopped",
      Self::Finished => "finished",
      Self::Workflow(WorkflowStatus::Backlog) => "backlog",
      Self::Workflow(WorkflowStatus::Todo) => "todo",
      Self::Workflow(WorkflowStatus::InProgress) => "in_progress",
      Self::Workflow(WorkflowStatus::InReview) => "in_review",
      Self::Workflow(WorkflowStatus::Done) => "done",
      Self::Workflow(WorkflowStatus::Canceled) => "canceled",
    }
  }
}

/// Some of a view's rows: a group's, or a flat view's all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grouped {
  /// `None` in a flat view.
  pub group: Option<Group>,
  pub rows: Vec<TaskRow>,
}

/// The rows of `view` among `rows`, in its groups and their order, each group in the rows' order; groups without rows
/// are left out. Done lists its rows newest first, unless `sort` orders them otherwise: the task that changed last
/// (closing it changes it) comes first.
pub fn grouped(
  view: ListView,
  rows: &[TaskRow],
  (data, waiting): (&ProjectTasks, &HashSet<TaskId>),
  sort: TaskSort,
) -> Vec<Grouped> {
  let mut kept: Vec<TaskRow> = rows.iter().filter(|row| view.keeps(row, waiting)).cloned().collect();
  let order: Vec<Group> = match view {
    ListView::Active => Group::ACTIVE.to_vec(),
    ListView::All => WorkflowStatus::ALL.map(Group::Workflow).to_vec(),
    ListView::Done if sort == TaskSort::Manual => {
      let updated = |row: &TaskRow| data.updated.get(&row.id).copied().unwrap_or(row.created_at);
      kept.sort_by_key(|row| std::cmp::Reverse(updated(row)));
      return flat(kept);
    }
    ListView::NeedsYou | ListView::Backlog | ListView::Done => return flat(kept),
  };
  let of = |row: &TaskRow| match view {
    ListView::Active => Group::next(row, waiting),
    _ => Group::Workflow(row.organization.status()),
  };
  order
    .into_iter()
    .map(|group| Grouped {
      group: Some(group),
      rows: kept.iter().filter(|row| of(row) == group).cloned().collect(),
    })
    .filter(|grouped| !grouped.rows.is_empty())
    .collect()
}

fn flat(rows: Vec<TaskRow>) -> Vec<Grouped> {
  if rows.is_empty() {
    return Vec::new();
  }
  vec![Grouped { group: None, rows }]
}
