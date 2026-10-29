//! *Delivery* on the Overview: each open module's progress with its forecast, by target date, and the queued work's
//! forecast, as `get_delivery` and the Modules list have them ([`crate::delivery`]). Read off the UI thread when the
//! project's tasks were saved, never per frame.

use std::{path::Path, sync::Arc};

use orchester_core::{id::ModuleId, module::ModuleState, store::TaskStore, time::Timestamp, usage::local_date};
use orchester_store::{ProjectDb, StoreError};

use crate::{
  delivery::{
    self, DeliveryFacts,
    forecast::{self, Forecast},
  },
  modules::data::{Progress, summaries},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleLine {
  pub id: ModuleId,
  pub name: Arc<str>,
  pub progress: Progress,
  pub forecast: Forecast,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverviewDelivery {
  /// The open modules, the nearest target date first, those without one after them by age.
  pub modules: Vec<ModuleLine>,
  /// The project's queued tasks still to be delivered.
  pub queued: Forecast,
}

impl OverviewDelivery {
  /// The delivery of the project at `root` as it stands at `now`.
  pub fn read_in(root: &Path, now: Timestamp) -> Result<Self, StoreError> {
    Self::read(&ProjectDb::open(root)?, now)
  }

  pub fn read(db: &ProjectDb, now: Timestamp) -> Result<Self, StoreError> {
    let tasks = db.tasks()?;
    let facts = DeliveryFacts::read(db, &tasks, now)?;
    let today = local_date(now);
    let history = delivery::history(&facts, today);
    let mut open: Vec<_> = summaries(&db.modules()?, &tasks)
      .into_iter()
      .filter(|summary| summary.module.details().state() == ModuleState::Open)
      .collect();
    open.sort_by_key(|summary| {
      (
        summary.module.details().target().is_none(),
        summary.module.details().target(),
      )
    });
    let modules = open
      .into_iter()
      .map(|summary| ModuleLine {
        id: summary.module.id(),
        name: summary.module.name().as_str().into(),
        forecast: Forecast::of(summary.progress.remaining(), &history, today, forecast::SEED),
        progress: summary.progress,
      })
      .collect();
    let queued = Forecast::of(delivery::queued_remaining(&facts), &history, today, forecast::SEED);
    Ok(Self { modules, queued })
  }
}
