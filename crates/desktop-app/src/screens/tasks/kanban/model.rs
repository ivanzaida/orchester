//! What the board shows, built from the rows the list reads (searched, filtered and ordered as the list orders them):
//! its columns, each with its cards in order, and its rows (swimlanes) when Display asks for them. The view picks the
//! columns (Active: Todo, In progress and In review; Backlog and Done their own; All every one) and keeps a column of
//! any status that holds a task of the view, so nothing the view holds goes missing. Canceled shows only while
//! Display shows it, or folded once a task was canceled from the board.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{
  id::ModuleId,
  task::organization::{Priority, WorkflowStatus},
};
use orchester_workspace::tasks::{
  data::ProjectTasks,
  list::{PipelineLabel, TaskRow},
};

use super::{
  card::CardModel,
  display::{Columns, Display, Lanes},
};
use crate::{
  routes::TaskListTab,
  ui::{
    icon::Icon,
    status::{priority_look, workflow_key, workflow_look},
  },
};

/// What a column holds: a workflow status, a priority, a module (or none) or a pipeline (or none).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColumnKey {
  Status(WorkflowStatus),
  Priority(Priority),
  Module(Option<ModuleId>),
  /// A pipeline by its name, whatever version a task pins; `None` for a task without a published one. Columns by
  /// pipeline only show.
  Pipeline(Option<Arc<str>>),
}

impl ColumnKey {
  /// Its name for element ids and the folded columns kept for the project.
  pub fn name(&self) -> String {
    match self {
      Self::Status(status) => status_name(*status).to_owned(),
      Self::Priority(priority) => format!("priority-{}", priority_name(*priority)),
      Self::Module(Some(module)) => format!("module-{module}"),
      Self::Module(None) => "module-none".to_owned(),
      Self::Pipeline(Some(name)) => format!("pipeline-{}", slug(name)),
      Self::Pipeline(None) => "pipeline-none".to_owned(),
    }
  }
}

/// One column.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnModel {
  pub key: ColumnKey,
  pub label: Arc<str>,
  pub glyph: Icon,
  pub color: PaletteColor,
  pub cards: Vec<CardModel>,
  /// How many of its cards wait for the person.
  pub needs_you: usize,
  /// How many tasks of the view it would hold without the search and the filters.
  pub total: usize,
}

/// One row (swimlane): its key, words, glyph, and its cards in each column, in the columns' order.
#[derive(Clone, Debug, PartialEq)]
pub struct LaneModel {
  pub key: String,
  pub label: Arc<str>,
  pub glyph: Icon,
  pub cells: Vec<Vec<CardModel>>,
  pub needs_you: usize,
}

impl LaneModel {
  pub fn count(&self) -> usize {
    self.cells.iter().map(Vec::len).sum()
  }
}

/// What the board is built from.
pub struct Source<'a> {
  pub view: TaskListTab,
  /// The view's rows as searched, filtered and ordered, and every row of the view whatever the search and filters.
  pub rows: &'a [CardModel],
  pub every: &'a [TaskRow],
  pub tasks: &'a ProjectTasks,
  pub display: &'a Display,
  /// A task was canceled from the board: the Canceled column shows folded with its count.
  pub canceled_now: bool,
}

/// The board's columns, in order.
pub fn columns(ctx: &Ctx, source: &Source<'_>) -> Vec<ColumnModel> {
  let keys = column_keys(source);
  keys
    .into_iter()
    .map(|key| {
      let cards: Vec<CardModel> = source
        .rows
        .iter()
        .filter(|card| key_of(&source.display.columns, &card.row) == key)
        .cloned()
        .collect();
      let total = source
        .every
        .iter()
        .filter(|row| key_of(&source.display.columns, row) == key)
        .count();
      let (label, glyph, color) = look(ctx, &key, source.tasks);
      ColumnModel {
        needs_you: cards.iter().filter(|card| card.waits).count(),
        key,
        label,
        glyph,
        color,
        cards,
        total,
      }
    })
    .filter(|column| {
      source.display.show_empty
        || !column.cards.is_empty()
        || matches!(column.key, ColumnKey::Status(WorkflowStatus::Canceled))
    })
    .collect()
}

/// A row of the board before its cards: its key, label and glyph, and which tasks it keeps.
type LaneOf = (String, Arc<str>, Icon, Box<dyn Fn(&TaskRow) -> bool>);

/// The board's rows, each with its cards in `columns`' order; `None` without rows.
pub fn lanes(
  ctx: &Ctx,
  display: &Display,
  (tasks, columns): (&ProjectTasks, &[ColumnModel]),
) -> Option<Vec<LaneModel>> {
  let lanes: Vec<LaneOf> = match display.lanes {
    Lanes::None => return None,
    Lanes::Module => {
      let mut lanes: Vec<LaneOf> = tasks
        .modules
        .iter()
        .map(|module| {
          let id = module.id();
          let lane: Box<dyn Fn(&TaskRow) -> bool> = Box::new(move |row: &TaskRow| row.module == Some(id));
          (
            format!("module-{id}"),
            Arc::from(module.name().as_str()),
            Icon::Target,
            lane,
          )
        })
        .collect();
      lanes.push((
        "module-none".to_owned(),
        ctx.t("tasks.filter.noModule"),
        Icon::Target,
        Box::new(|row: &TaskRow| row.module.is_none()),
      ));
      lanes
    }
    Lanes::Priority => Priority::ALL
      .into_iter()
      .map(|priority| {
        let (glyph, key, _) = priority_look(priority);
        let lane: Box<dyn Fn(&TaskRow) -> bool> =
          Box::new(move |row: &TaskRow| row.organization.priority() == priority);
        (lane_key_of_priority(priority), ctx.t(key), glyph, lane)
      })
      .collect(),
  };
  Some(
    lanes
      .into_iter()
      .map(|(key, label, glyph, keeps)| {
        let cells: Vec<Vec<CardModel>> = columns
          .iter()
          .map(|column| column.cards.iter().filter(|card| keeps(&card.row)).cloned().collect())
          .collect();
        let needs_you = cells.iter().flatten().filter(|card| card.waits).count();
        LaneModel {
          key,
          label,
          glyph,
          cells,
          needs_you,
        }
      })
      .filter(|lane| lane.count() > 0)
      .collect(),
  )
}

/// The columns of the view, in their order.
fn column_keys(source: &Source<'_>) -> Vec<ColumnKey> {
  match source.display.columns {
    Columns::Status => status_columns(source).into_iter().map(ColumnKey::Status).collect(),
    Columns::Priority => Priority::ALL.into_iter().map(ColumnKey::Priority).collect(),
    Columns::Module => source
      .tasks
      .modules
      .iter()
      .map(|module| ColumnKey::Module(Some(module.id())))
      .chain([ColumnKey::Module(None)])
      .collect(),
    Columns::Pipeline => {
      let mut keys: Vec<ColumnKey> = Vec::new();
      for row in source.every {
        let key = ColumnKey::Pipeline(pipeline_name(&row.pipeline));
        if !keys.contains(&key) {
          keys.push(key);
        }
      }
      keys
    }
  }
}

/// The workflow statuses the view shows as columns, and any other that holds a task of the view.
fn status_columns(source: &Source<'_>) -> Vec<WorkflowStatus> {
  WorkflowStatus::ALL
    .into_iter()
    .filter(|status| {
      let own = owns(source.view, *status);
      let holds = || source.every.iter().any(|row| row.organization.status() == *status);
      match status {
        WorkflowStatus::Canceled => {
          (source.display.show_canceled || source.canceled_now) && (own || holds() || source.view == TaskListTab::All)
        }
        _ => own || holds(),
      }
    })
    .collect()
}

/// Whether `view` shows `status` as a column of its own, whatever its tasks.
pub fn owns(view: TaskListTab, status: WorkflowStatus) -> bool {
  match view {
    TaskListTab::Active => matches!(
      status,
      WorkflowStatus::Todo | WorkflowStatus::InProgress | WorkflowStatus::InReview
    ),
    TaskListTab::Backlog => status == WorkflowStatus::Backlog,
    TaskListTab::Done => status == WorkflowStatus::Done,
    TaskListTab::All => status != WorkflowStatus::Canceled,
  }
}

/// The column `row` belongs in.
pub fn key_of(columns: &Columns, row: &TaskRow) -> ColumnKey {
  match columns {
    Columns::Status => ColumnKey::Status(row.organization.status()),
    Columns::Priority => ColumnKey::Priority(row.organization.priority()),
    Columns::Module => ColumnKey::Module(row.module),
    Columns::Pipeline => ColumnKey::Pipeline(pipeline_name(&row.pipeline)),
  }
}

fn look(ctx: &Ctx, key: &ColumnKey, tasks: &ProjectTasks) -> (Arc<str>, Icon, PaletteColor) {
  match key {
    ColumnKey::Status(status) => {
      let (glyph, color) = workflow_look(*status);
      (ctx.t(workflow_key(*status)), glyph, color)
    }
    ColumnKey::Priority(priority) => {
      let (glyph, word, color) = priority_look(*priority);
      (ctx.t(word), glyph, color)
    }
    ColumnKey::Module(Some(id)) => (
      tasks.modules.iter().find(|module| module.id() == *id).map_or_else(
        || Arc::from(id.to_string().as_str()),
        |module| Arc::from(module.name().as_str()),
      ),
      Icon::Target,
      PaletteColor::TextMuted,
    ),
    ColumnKey::Module(None) => (ctx.t("tasks.filter.noModule"), Icon::Target, PaletteColor::TextMuted),
    ColumnKey::Pipeline(Some(name)) => (name.clone(), Icon::Workflow, PaletteColor::TextMuted),
    ColumnKey::Pipeline(None) => (
      ctx.t("tasks.filter.noPipeline"),
      Icon::Workflow,
      PaletteColor::TextMuted,
    ),
  }
}

/// A pipeline's name, the same whatever version a task pins, or the slug of a shared task's pipeline this computer
/// lacks; `None` without a published one.
fn pipeline_name(label: &PipelineLabel) -> Option<Arc<str>> {
  match label {
    PipelineLabel::Published { name, .. } => Some(name.clone()),
    PipelineLabel::Missing { slug } => Some(slug.clone()),
    PipelineLabel::NotChosen | PipelineLabel::Unavailable => None,
  }
}

pub const fn status_name(status: WorkflowStatus) -> &'static str {
  match status {
    WorkflowStatus::Backlog => "backlog",
    WorkflowStatus::Todo => "todo",
    WorkflowStatus::InProgress => "in_progress",
    WorkflowStatus::InReview => "in_review",
    WorkflowStatus::Done => "done",
    WorkflowStatus::Canceled => "canceled",
  }
}

pub const fn priority_name(priority: Priority) -> &'static str {
  match priority {
    Priority::Urgent => "urgent",
    Priority::High => "high",
    Priority::Medium => "medium",
    Priority::Low => "low",
    Priority::None => "none",
  }
}

/// The key of the row of `priority`'s tasks.
pub fn lane_key_of_priority(priority: Priority) -> String {
  format!("priority-{}", priority_name(priority))
}

/// `words` as an element id's part: lower case, other than letters and digits a dash.
fn slug(words: &str) -> String {
  words
    .chars()
    .map(|letter| {
      if letter.is_alphanumeric() {
        letter.to_ascii_lowercase()
      } else {
        '-'
      }
    })
    .collect()
}
