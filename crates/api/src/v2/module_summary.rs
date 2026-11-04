//! A module's progress, forecast, burndown and history as S10 reads, counted by core's rules
//! (`orchester_core::module::{progress, burndown}`, `orchester_core::forecast`) so a server's summaries say what the
//! desktop's Modules pages say. Days are `YYYY-MM-DD` calendar days; which time zone a server counts them in is not
//! decided by this contract.

use chrono::NaiveDate;
use orchester_core::{
  forecast::{Forecast, Outlook},
  id::ModuleId,
  module::{
    ModuleState,
    burndown::{Burndown, BurndownDay},
    measure::WorkUnit,
    progress::Progress,
  },
};
use serde::{Deserialize, Serialize};

use super::{EventCursor, ModuleRead, Page, PageRequest, page::nullable};
use crate::{SharedModuleChange, wire};

#[derive(Serialize, Deserialize)]
#[serde(remote = "WorkUnit", rename_all = "snake_case")]
enum WorkUnitWire {
  Tasks,
}

/// How far a module's work is: its scope (its tasks that are not Canceled) and what of it is Done, in `unit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleProgressRead {
  #[serde(with = "WorkUnitWire")]
  pub unit: WorkUnit,
  pub scope: u32,
  pub done: u32,
}

impl ModuleProgressRead {
  pub const fn from_core(progress: Progress) -> Self {
    Self {
      unit: progress.unit,
      scope: progress.scope,
      done: progress.done,
    }
  }

  /// Core's progress, whose percent and remaining work a reader shows.
  pub const fn to_core(self) -> Progress {
    Progress {
      unit: self.unit,
      scope: self.scope,
      done: self.done,
    }
  }
}

/// What a forecast says: nothing left, too little history, or the 50% (`likely`) and 85% (`safe`) days.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OutlookRead {
  /// Braced so serde refuses unknown fields on it too; a unit variant would ignore them.
  Done {},
  NotEnoughHistory {},
  Dates {
    likely: String,
    safe: String,
    past_horizon: bool,
  },
}

/// When a module's remaining work is likely delivered, with the history the forecast rests on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForecastRead {
  pub remaining: u32,
  pub history_days: u64,
  pub history_delivered: u32,
  pub outlook: OutlookRead,
}

impl ForecastRead {
  pub fn from_core(forecast: &Forecast) -> Self {
    let outlook = match &forecast.outlook {
      Outlook::Done => OutlookRead::Done {},
      Outlook::NotEnoughHistory => OutlookRead::NotEnoughHistory {},
      Outlook::Dates {
        likely,
        safe,
        past_horizon,
      } => OutlookRead::Dates {
        likely: day(*likely),
        safe: day(*safe),
        past_horizon: *past_horizon,
      },
    };
    Self {
      remaining: forecast.remaining,
      history_days: forecast.history_days,
      history_delivered: forecast.history_delivered,
      outlook,
    }
  }
}

/// One day of a module's burndown: `scope` and `remaining` are `null` for a day to come and before the module had work,
/// `ideal` without a target date and before the module had work.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BurndownDayRead {
  pub date: String,
  #[serde(deserialize_with = "nullable")]
  pub scope: Option<u32>,
  #[serde(deserialize_with = "nullable")]
  pub remaining: Option<u32>,
  #[serde(deserialize_with = "nullable")]
  pub ideal: Option<f64>,
  pub added: u32,
  pub removed: u32,
}

impl BurndownDayRead {
  pub fn from_core(burndown_day: &BurndownDay) -> Self {
    Self {
      date: day(burndown_day.date),
      scope: burndown_day.scope,
      remaining: burndown_day.remaining,
      ideal: burndown_day.ideal,
      added: burndown_day.added,
      removed: burndown_day.removed,
    }
  }
}

/// A module's burndown by day, at most a year of days (`orchester_core::module::burndown::MAX_CHART_DAYS`), with its
/// forecast, at snapshot `cursor`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleBurndownRead {
  pub module: ModuleId,
  #[serde(with = "WorkUnitWire")]
  pub unit: WorkUnit,
  pub start_scope: u32,
  pub days: Vec<BurndownDayRead>,
  pub forecast: ForecastRead,
  pub cursor: EventCursor,
}

impl ModuleBurndownRead {
  pub fn from_core(module: ModuleId, (burndown, forecast): (&Burndown, &Forecast), cursor: EventCursor) -> Self {
    Self {
      module,
      unit: burndown.unit,
      start_scope: burndown.start_scope,
      days: burndown.days.iter().map(BurndownDayRead::from_core).collect(),
      forecast: ForecastRead::from_core(forecast),
      cursor,
    }
  }
}

/// A module of the Modules list: the module, its progress, how many tasks it has (Canceled ones too) and its forecast.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleSummary {
  pub module: ModuleRead,
  pub progress: ModuleProgressRead,
  pub tasks: u32,
  pub forecast: ForecastRead,
}

/// `{state?, page}`: one page of the modules, the open or the closed ones when `state` names one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleListRequest {
  #[serde(default, with = "optional_state")]
  pub state: Option<ModuleState>,
  #[serde(default)]
  pub page: PageRequest,
}

/// A module's history: every move of a task into or out of it, in ascending `sequence`.
pub type ModuleHistoryPage = Page<SharedModuleChange>;

fn day(date: NaiveDate) -> String {
  date.format("%Y-%m-%d").to_string()
}

mod optional_state {
  use super::*;

  #[derive(Serialize, Deserialize)]
  struct Value(#[serde(with = "wire::ModuleStateWire")] ModuleState);

  pub fn serialize<S: serde::Serializer>(value: &Option<ModuleState>, serializer: S) -> Result<S::Ok, S::Error> {
    value.map(Value).serialize(serializer)
  }

  pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<ModuleState>, D::Error> {
    Ok(Option::<Value>::deserialize(deserializer)?.map(|value| value.0))
  }
}
