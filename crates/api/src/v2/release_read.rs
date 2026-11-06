//! A project's releases as S10 reads (`docs/architecture/server.md`, "Releases (team authority)"): the release list
//! with each release's progress and forecast against its planned day, one release, its progress, its burndown with the
//! forecast, its history and its changelog. Progress, burndown and forecast are counted by core's module rules over the
//! release's tasks (Canceled out of scope, Done done, every move into or out of the release on its day). Every field
//! is sent; one without a value is `null`, and an unknown field is refused. Which time zone a server counts days in is
//! not decided by this contract.

use orchester_core::{
  forecast::Forecast,
  id::ReleaseId,
  module::{burndown::Burndown, measure::WorkUnit},
  release::ReleaseState,
};
use serde::{Deserialize, Serialize};

use super::{
  BurndownDayRead, Capabilities, EventCursor, ForecastRead, ModuleProgressRead, Page, PageRequest,
  module_summary::WorkUnitWire, page::nullable,
};
use crate::{ReleaseChange, SharedRelease};

/// A release with the caller's capabilities on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseRead {
  pub release: SharedRelease,
  pub capabilities: Capabilities,
}

/// A release of the Releases list: the release, its progress, how many tasks it has (Canceled ones too), its forecast,
/// whether that forecast's 85% (`safe`) day is after its planned day, and how many days past its planned day a planned
/// release is, `null` when it is not overdue. A released release is never after its planned day nor overdue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseSummary {
  pub release: ReleaseRead,
  pub progress: ModuleProgressRead,
  pub tasks: u32,
  pub forecast: ForecastRead,
  pub after_planned: bool,
  #[serde(deserialize_with = "nullable")]
  pub overdue_days: Option<u32>,
}

/// `{state?, page}`: one page of the releases, the planned or the released ones when `state` names one. Releases come
/// in ascending (creation sequence, id) order, as every S10 listing; a reader orders them as the Releases view does.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseListRequest {
  #[serde(default)]
  pub state: Option<ReleaseState>,
  #[serde(default)]
  pub page: PageRequest,
}

/// A release's burndown by day from its first task to its planned day, at most a year of days, with its forecast, at
/// snapshot `cursor`; without a planned day it has no ideal line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseBurndownRead {
  pub release: ReleaseId,
  #[serde(with = "WorkUnitWire")]
  pub unit: WorkUnit,
  pub start_scope: u32,
  pub days: Vec<BurndownDayRead>,
  pub forecast: ForecastRead,
  pub cursor: EventCursor,
}

impl ReleaseBurndownRead {
  pub fn from_core(release: ReleaseId, (burndown, forecast): (&Burndown, &Forecast), cursor: EventCursor) -> Self {
    Self {
      release,
      unit: burndown.unit,
      start_scope: burndown.start_scope,
      days: burndown.days.iter().map(BurndownDayRead::from_core).collect(),
      forecast: ForecastRead::from_core(forecast),
      cursor,
    }
  }
}

/// A release's history: every move of a task into or out of it, in ascending `sequence`.
pub type ReleaseHistoryPage = Page<ReleaseChange>;
