//! What the @ menu offers (`docs/design/mentions.md`, "Order" and "Filter"): the project's tasks with their workflow
//! status and its runs with their state, most recent first, and how what the person typed after the `@` filters them,
//! by key, title or run number, ignoring case. In a run's Conversation its own task and run lead. The repository's
//! files come from their own index ([`super::files`]).

use std::{path::Path, sync::Arc};

use orchester_core::{
  project::TaskKey,
  run::{RunNumber, TaskPhase},
  store::{MergeStore, RunStore, TaskStore},
  task::{Task, organization::WorkflowStatus},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{Mention, files::FileChoice};
use crate::{
  messages::{self, Message},
  tasks::detail::run_store_error,
};

/// How many rows of a section the menu shows before anything is typed (`docs/design/mentions.md`, "Menu").
pub const BEFORE_TYPING: usize = 3;
/// How many matching rows of a section the menu shows once something is typed.
pub const MATCHING: usize = 5;

/// How many rows of a section the menu shows for `query`, what was typed after the `@`.
pub const fn limit(query: &str) -> usize {
  if query.is_empty() { BEFORE_TYPING } else { MATCHING }
}

/// A task the menu offers, with what a mention of it tells the agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskChoice {
  pub key: TaskKey,
  pub title: Arc<str>,
  pub status: WorkflowStatus,
  pub brief: Arc<str>,
  /// When the task last changed, which orders the menu.
  pub at: Timestamp,
}

/// A run the menu offers, with what a mention of it tells the agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunChoice {
  pub key: TaskKey,
  /// Its task's title.
  pub title: Arc<str>,
  pub number: RunNumber,
  pub phase: TaskPhase,
  pub branch: Arc<str>,
}

impl RunChoice {
  pub fn mention(&self) -> Mention {
    Mention::Run(self.key.clone(), self.number)
  }
}

impl TaskChoice {
  pub fn mention(&self) -> Mention {
    Mention::Task(self.key.clone())
  }
}

/// The rows of one section the menu shows, and how many there are in all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section<T> {
  pub shown: Vec<T>,
  pub total: usize,
}

impl<T> Default for Section<T> {
  fn default() -> Self {
    Self {
      shown: Vec::new(),
      total: 0,
    }
  }
}

/// What the menu shows for what was typed: tasks, runs and, once the repository's files are listed
/// ([`Offer::with_files`]), files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
  pub tasks: Section<TaskChoice>,
  pub runs: Section<RunChoice>,
  pub files: Section<FileChoice>,
}

impl Offer {
  /// The offer with `files`, the repository's files that match.
  #[must_use]
  pub fn with_files(self, files: Section<FileChoice>) -> Self {
    Self { files, ..self }
  }

  /// Whether nothing matches.
  pub fn is_empty(&self) -> bool {
    self.tasks.shown.is_empty() && self.runs.shown.is_empty() && self.files.shown.is_empty()
  }

  /// How many rows match in all.
  pub const fn matches(&self) -> usize {
    self.tasks.total + self.runs.total + self.files.total
  }

  /// The mentions of the rows shown, in the menu's order: what the arrow keys move through.
  pub fn mentions(&self) -> Vec<Mention> {
    let tasks = self.tasks.shown.iter().map(TaskChoice::mention);
    let runs = self.runs.shown.iter().map(RunChoice::mention);
    tasks
      .chain(runs)
      .chain(self.files.shown.iter().map(FileChoice::mention))
      .collect()
  }
}

/// The project's tasks, most recently changed first, and its runs, latest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
  tasks: Arc<Vec<TaskChoice>>,
  runs: Arc<Vec<RunChoice>>,
}

impl Catalog {
  /// Reads the tasks and runs of the project at `root`. Blocks on SQLite.
  pub fn read_in(root: &Path) -> Result<Self, Message> {
    ProjectDb::open(root)
      .and_then(|db| Self::read(&db))
      .map_err(|error| messages::store_error(&error))
  }

  /// Reads the tasks and runs `db` holds; a run that cannot be read is left out.
  pub fn read(db: &ProjectDb) -> Result<Self, StoreError> {
    let mut tasks = Vec::new();
    let mut runs = Vec::new();
    for task in db.tasks()? {
      let at = db.task_organization_updated_at(task.id())?.unwrap_or(task.created_at());
      for run in db
        .runs_of_task(task.id())
        .map_err(run_store_error)?
        .into_iter()
        .flatten()
      {
        let merge = db.run_merge(run.id())?;
        runs.push(RunChoice {
          key: task.key().clone(),
          title: title(&task),
          number: run.number(),
          phase: TaskPhase::of(&task, Some(&run), merge.as_ref()),
          branch: run.branch().as_str().into(),
        });
      }
      tasks.push(TaskChoice {
        key: task.key().clone(),
        title: title(&task),
        status: task.organization().status(),
        brief: task.details().brief.as_str().into(),
        at,
      });
    }
    tasks.sort_by_key(|task| std::cmp::Reverse(task.at));
    runs.sort_by_key(|run| std::cmp::Reverse(run.number));
    Ok(Self {
      tasks: Arc::new(tasks),
      runs: Arc::new(runs),
    })
  }

  pub fn task(&self, key: &TaskKey) -> Option<&TaskChoice> {
    self.tasks.iter().find(|task| &task.key == key)
  }

  pub fn run(&self, key: &TaskKey, number: RunNumber) -> Option<&RunChoice> {
    self.runs.iter().find(|run| &run.key == key && run.number == number)
  }

  /// What the menu shows for `query`, typed after the `@`; `own`, the task a run's Conversation shows and its run,
  /// leads its sections.
  pub fn offer(&self, query: &str, own: Option<(&TaskKey, Option<RunNumber>)>) -> Offer {
    let query = query.to_lowercase();
    let shown = limit(&query);
    let own_task = |key: &TaskKey| own.is_some_and(|(own, _)| own == key);
    let own_run = |run: &RunChoice| own.is_some_and(|(key, number)| key == &run.key && number == Some(run.number));
    let tasks = self
      .tasks
      .iter()
      .filter(|task| task_matches(&query, &task.key, &task.title));
    let runs = self.runs.iter().filter(|run| {
      let number = run.number.get().to_string();
      matches(&query, &[&run.mention().written()[1..], &run.title, &number])
    });
    Offer {
      tasks: section(tasks, shown, |task| own_task(&task.key)),
      runs: section(runs, shown, own_run),
      files: Section::default(),
    }
  }
}

/// The section of `rows`, the one `leads` says first, up to `limit` of them.
fn section<'a, T: Clone + 'a>(
  rows: impl Iterator<Item = &'a T>,
  limit: usize,
  leads: impl Fn(&T) -> bool,
) -> Section<T> {
  let (mut first, rest): (Vec<&T>, Vec<&T>) = rows.partition(|row| leads(row));
  let total = first.len() + rest.len();
  first.extend(rest);
  Section {
    shown: first.into_iter().take(limit).cloned().collect(),
    total,
  }
}

/// Whether the task `key` titled `title` matches `query`, already in lower case, by its key or its title: the @ menu's
/// search, which the /watch-task picker shares.
pub(crate) fn task_matches(query: &str, key: &TaskKey, title: &str) -> bool {
  matches(query, &[&key.to_string(), title])
}

/// Whether one of `fields` holds `query`, already in lower case; everything holds nothing.
pub(crate) fn matches(query: &str, fields: &[&str]) -> bool {
  query.is_empty() || fields.iter().any(|field| field.to_lowercase().contains(query))
}

fn title(task: &Task) -> Arc<str> {
  task.details().title.as_str().into()
}
