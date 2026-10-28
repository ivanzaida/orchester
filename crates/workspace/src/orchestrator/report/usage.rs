//! The day's tokens and active agent time in the daily report, as `get_usage` reads them (`crate::usage`): in total, by
//! task and by agent provider, most spent first, against the project's daily budget, each linking to the Usage page of
//! the day narrowed to it.

use std::slice;

use orchester_core::{
  id::ProjectId,
  orchestrator::report::{DayUsage, ProviderShare, SharedBy, TaskShare},
  usage::{
    DayRange, GroupBy, GroupKey, Grouped, Grouping, Period, SpentOn, UsageFilter, UsageQuery as ReportQuery, grouped,
  },
};

use super::facts::DayFacts;
use crate::{links::Link, usage::UsageQuery};

/// The usage of `facts`' day in project `project`.
pub(super) fn of(facts: &DayFacts, project: ProjectId) -> DayUsage {
  let range = DayRange::new(facts.day, facts.day).expect("a day is a range of one day");
  let filter = UsageFilter::default();
  let query = || ReportQuery {
    range,
    grouping: Grouping::Hour,
    filter: &filter,
    now: facts.now,
  };
  let projects = slice::from_ref(&facts.usage);
  let Grouped { summary, rows } = grouped(projects, query(), GroupBy::Task);
  let page = |filter: UsageFilter| {
    let shown = UsageQuery {
      period: Period::Custom(range),
      group: GroupBy::Hour,
      filter,
    };
    Link::Usage(project, Box::new(shown)).path()
  };
  let tasks = rows
    .iter()
    .filter_map(|row| match &row.key {
      GroupKey::Task { key, title, .. } => Some(TaskShare {
        task: key.to_string(),
        title: title.to_string(),
        spent: row.usage.counted(),
        active_ms: row.agent_ms,
        link: page(UsageFilter {
          tasks: vec![key.clone()],
          ..UsageFilter::default()
        }),
      }),
      _ => None,
    })
    .collect();
  let providers = grouped(projects, query(), GroupBy::Provider)
    .rows
    .iter()
    .filter_map(|row| {
      let (by, filter) = shared_by(facts, &row.key)?;
      Some(ProviderShare {
        by,
        spent: row.usage.counted(),
        active_ms: row.agent_ms,
        link: page(filter),
      })
    })
    .collect();
  DayUsage {
    spent: summary.usage.counted(),
    active_ms: summary.agent_ms,
    budget: facts.budget,
    tasks,
    providers,
    link: page(UsageFilter::default()),
  }
}

/// What a row grouped by provider was spent on, with the filter its link narrows the page to; command tools' time
/// narrows nothing.
fn shared_by(facts: &DayFacts, key: &GroupKey) -> Option<(SharedBy, UsageFilter)> {
  Some(match key {
    GroupKey::Provider(SpentOn::Provider(connection)) => {
      let known = facts.usage.facts.providers.iter().find(|known| known.id == *connection);
      let by = SharedBy::Provider {
        connection: *connection,
        name: known.map(|known| known.name.to_string()),
        removed: known.is_none_or(|known| known.removed),
      };
      let filter = UsageFilter {
        providers: vec![*connection],
        ..UsageFilter::default()
      };
      (by, filter)
    }
    GroupKey::Provider(SpentOn::Earlier) => (
      SharedBy::Earlier,
      UsageFilter {
        earlier: true,
        ..UsageFilter::default()
      },
    ),
    GroupKey::CommandTools => (SharedBy::CommandTools, UsageFilter::default()),
    _ => return None,
  })
}
