//! The day's tokens and active agent time in the daily report, as `get_usage` reads them (`crate::usage`): in total, by
//! task and by account, most spent first, against the project's daily budget, each linking to the Usage page of the
//! day narrowed to it.

use std::slice;

use orchester_core::{
  id::ProjectId,
  orchestrator::report::{AccountShare, DayUsage, TaskShare},
  usage::{DayRange, GroupBy, GroupKey, Grouped, Grouping, Period, UsageFilter, UsageQuery as ReportQuery, grouped},
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
  let accounts = grouped(projects, query(), GroupBy::Account)
    .rows
    .iter()
    .filter_map(|row| match &row.key {
      GroupKey::Account(account) => Some(AccountShare {
        account: *account,
        name: account.and_then(|id| {
          facts
            .accounts
            .iter()
            .find(|known| known.id() == id)
            .and_then(|known| known.name())
            .map(|name| name.as_str().to_owned())
        }),
        spent: row.usage.counted(),
        active_ms: row.agent_ms,
        link: page(match account {
          Some(id) => UsageFilter {
            accounts: vec![*id],
            ..UsageFilter::default()
          },
          None => UsageFilter {
            no_account: true,
            ..UsageFilter::default()
          },
        }),
      }),
      _ => None,
    })
    .collect();
  DayUsage {
    spent: summary.usage.counted(),
    active_ms: summary.agent_ms,
    budget: facts.budget,
    tasks,
    accounts,
    link: page(UsageFilter::default()),
  }
}
