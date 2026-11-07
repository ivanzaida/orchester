//! A project's modules as the Modules list, a module's page and the tools read them: each module with its progress,
//! and one module with its tasks and its burndown. Every function blocks on SQLite; the UI calls them off the UI
//! thread.

use std::{
  collections::{BTreeSet, HashMap},
  sync::Arc,
};

use chrono::NaiveDate;
pub use orchester_core::module::progress::Progress;
use orchester_core::{
  id::{ModuleId, TaskId},
  module::Module,
  store::TaskStore,
  task::Task,
  time::Timestamp,
  usage::local_date,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  burndown::{Burndown, ChartDays, TaskTrack},
  measure::Measure,
};
use crate::delivery::{
  self, DeliveryFacts,
  forecast::{self, Forecast},
};

/// A module of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleSummary {
  pub module: Module,
  pub progress: Progress,
  /// How many tasks are in it, Canceled ones too.
  pub tasks: usize,
}

/// The modules of `tasks`' project, oldest first, with their progress.
pub fn summaries(modules: &[Module], tasks: &[Task]) -> Vec<ModuleSummary> {
  modules
    .iter()
    .map(|module| {
      let members: Vec<&Task> = module_tasks(module.id(), tasks).collect();
      let measure = Measure::of(members.iter().copied());
      ModuleSummary {
        module: module.clone(),
        progress: Progress::of(members.iter().copied(), &measure),
        tasks: members.len(),
      }
    })
    .collect()
}

/// The tasks of `tasks` that are in module `id`, Canceled ones too: what the Modules list counts in a module, and a
/// line about watching it (`crate::orchestrator::watching::WatchedTasks`).
pub fn module_tasks(id: ModuleId, tasks: &[Task]) -> impl Iterator<Item = &Task> {
  tasks.iter().filter(move |task| task.module() == Some(id))
}

/// When the remaining work of each module of the project in `db` is likely delivered after `now`, by the project's
/// recent throughput, as a module's page forecasts it; the Modules list shows it in each row.
pub fn forecasts(db: &ProjectDb, now: Timestamp) -> Result<HashMap<ModuleId, Forecast>, StoreError> {
  let tasks = db.tasks()?;
  let today = local_date(now);
  let delivered = delivery::history(&DeliveryFacts::read(db, &tasks, now)?, today);
  Ok(
    summaries(&db.modules()?, &tasks)
      .into_iter()
      .map(|summary| {
        let forecast = Forecast::of(summary.progress.remaining(), &delivered, today, forecast::SEED);
        (summary.module.id(), forecast)
      })
      .collect(),
  )
}

/// A module's page: the module, its tasks in the list's order, its progress, its burndown and when its remaining work
/// is likely delivered (`crate::delivery::forecast`).
#[derive(Clone, Debug, PartialEq)]
pub struct ModulePage {
  pub module: Module,
  pub tasks: Arc<Vec<Task>>,
  pub progress: Progress,
  pub burndown: Burndown,
  pub forecast: Forecast,
}

impl ModulePage {
  /// Module `id` of the project in `db` as it stands at `now`; `None` when it does not exist.
  pub fn read(db: &ProjectDb, id: ModuleId, now: Timestamp) -> Result<Option<Self>, StoreError> {
    let Some(module) = db.module(id)? else {
      return Ok(None);
    };
    let all = db.tasks()?;
    // The forecast rests on the whole project's recent throughput, as the Delivery view's does.
    let delivered = delivery::history(&DeliveryFacts::read(db, &all, now)?, local_date(now));
    let changes = db.module_changes(id)?;
    // Every task that was ever in the module counts on the days it was in it.
    let tracked: BTreeSet<TaskId> = changes
      .iter()
      .map(|change| change.task)
      .chain(all.iter().filter(|task| task.module() == Some(id)).map(Task::id))
      .collect();
    let tracked_tasks: Vec<&Task> = all.iter().filter(|task| tracked.contains(&task.id())).collect();
    let measure = Measure::of(tracked_tasks.iter().copied());
    let mut tracks = Vec::with_capacity(tracked_tasks.len());
    for task in &tracked_tasks {
      let history = db.task_organization_history(task.id())?;
      tracks.push(TaskTrack {
        task: task.id(),
        weight: measure.weight(task.id()),
        membership: changes
          .iter()
          .filter(|change| change.task == task.id())
          .map(|change| (change.at, change.to == Some(id)))
          .collect(),
        first_status: history
          .first()
          .map_or(task.organization().status(), |change| change.from.status()),
        statuses: history
          .iter()
          .filter(|change| change.from.status() != change.to.status())
          .map(|change| (change.at, change.to.status()))
          .collect(),
      });
    }
    let members: Vec<Task> = all.into_iter().filter(|task| task.module() == Some(id)).collect();
    let days = ChartDays {
      first: first_day(&module, changes.first().map(|change| change.at)),
      target: module.details().target(),
      today: local_date(now),
    };
    let progress = Progress::of(&members, &measure);
    Ok(Some(Self {
      burndown: Burndown::of(&tracks, measure.unit, days),
      forecast: Forecast::of(progress.remaining(), &delivered, days.today, forecast::SEED),
      progress,
      tasks: Arc::new(members),
      module,
    }))
  }
}

/// The chart's first day: the module's start, or else the day it got its first task, or else the day it was made.
fn first_day(module: &Module, first_change: Option<Timestamp>) -> NaiveDate {
  module
    .details()
    .start()
    .unwrap_or_else(|| local_date(first_change.unwrap_or(module.created_at())))
}
