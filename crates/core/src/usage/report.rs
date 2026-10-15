//! The dashboard's figures for a range and the filters: the turns that count, the reviews of auto mode's
//! reviewer among them, are matched to their invocation and run once, then each section reads them.

use std::{collections::HashMap, sync::Arc};

use super::{
  Breakdowns, Efficiency, Grouping, RunCost, SeriesBar, TaskCost, UsageFilter,
  facts::{InvocationFacts, ProjectFacts, RunFacts},
  period::DayRange,
};
use crate::{
  connection::AgentProvider,
  id::{InvocationId, ProjectId, RunId},
  run::TokenUsage,
  time::Timestamp,
};

/// What the dashboard asks for.
#[derive(Clone, Copy, Debug)]
pub struct UsageQuery<'a> {
  pub range: DayRange,
  pub grouping: Grouping,
  pub filter: &'a UsageFilter,
  /// The time invocations that still run are measured to.
  pub now: Timestamp,
}

/// The Summary's figures for one range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
  pub usage: TokenUsage,
  /// Runs with agent activity in the range.
  pub runs: usize,
  pub finished: usize,
  pub failed: usize,
  /// Active agent time in the range.
  pub agent_ms: u64,
  /// Tokens spent in the range by runs that failed.
  pub failed_spent: u64,
}

impl Summary {
  /// Input − cached input + output.
  pub const fn spent(&self) -> u64 {
    self.usage.counted()
  }

  /// Cached input of all input, in whole percent; `None` without input.
  pub fn cache_share(&self) -> Option<u64> {
    super::percent(self.usage.cached_input, self.usage.input)
  }

  /// Failed runs of all runs, in whole percent; `None` without runs.
  pub fn failed_share(&self) -> Option<u64> {
    let count = |count: usize| u64::try_from(count).unwrap_or(u64::MAX);
    super::percent(count(self.failed), count(self.runs))
  }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsageReport {
  pub summary: Summary,
  /// The same figures for the as many days before.
  pub previous: Summary,
  pub series: Vec<SeriesBar>,
  pub breakdowns: Breakdowns,
  /// The tasks with runs in the range, most spent first.
  pub tasks: Vec<TaskCost>,
  /// The runs with activity in the range, most spent first.
  pub runs: Vec<RunCost>,
  pub efficiency: Efficiency,
  /// When the latest turn of any listed project completed, filters aside.
  pub last_turn: Option<Timestamp>,
}

impl UsageReport {
  pub fn build(projects: &[ProjectFacts], query: UsageQuery<'_>) -> Self {
    let matched = Matched::of(projects, query.filter);
    let current = matched.within(query.range, query.now);
    let previous = matched.within(query.range.previous(), query.now);
    let summary = current.summary();
    Self {
      summary,
      previous: previous.summary(),
      series: super::series::bars(&current, query.range, query.grouping),
      breakdowns: Breakdowns::of(&current, projects),
      tasks: super::expensive::tasks(&current),
      runs: super::expensive::runs(&current),
      efficiency: Efficiency::of(&current, query.range, &summary),
      last_turn: projects.iter().filter_map(|project| project.facts.last_turn).max(),
    }
  }

  /// Whether the range has activity that the filters hide: the filters match nothing.
  pub fn hidden_by(projects: &[ProjectFacts], query: UsageQuery<'_>) -> bool {
    let all = Matched::of(projects, &UsageFilter::default());
    !query.filter.is_empty() && all.within(query.range, query.now).has_activity()
  }
}

/// One invocation that counts, with its run and project.
#[derive(Clone, Copy, Debug)]
pub(super) struct Launch<'a> {
  pub project: &'a ProjectFacts,
  pub run: &'a RunFacts,
  pub invocation: &'a InvocationFacts,
}

/// Who spent a turn's tokens.
#[derive(Clone, Copy, Debug)]
pub(super) enum Spender<'a> {
  /// The invocation's agent.
  Agent,
  /// Auto mode's reviewer of the invocation's requests.
  Reviewer { cli: AgentProvider, model: &'a Arc<str> },
}

/// One turn that counts: an agent's, or a review of its requests.
#[derive(Clone, Copy, Debug)]
pub(super) struct Turn<'a> {
  pub launch: Launch<'a>,
  pub at: Timestamp,
  pub usage: TokenUsage,
  pub by: Spender<'a>,
}

impl<'a> Turn<'a> {
  pub const fn run(&self) -> &'a RunFacts {
    self.launch.run
  }

  /// The CLI whose tokens these are.
  pub const fn cli(&self) -> Option<AgentProvider> {
    match self.by {
      Spender::Agent => self.launch.invocation.cli,
      Spender::Reviewer { cli, .. } => Some(cli),
    }
  }

  /// The model that spent them.
  pub fn model(&self) -> Option<&'a Arc<str>> {
    match self.by {
      Spender::Agent => self.launch.invocation.model.as_ref(),
      Spender::Reviewer { model, .. } => Some(model),
    }
  }
}

/// Every turn and invocation the filters admit, in all ranges.
pub(super) struct Matched<'a> {
  turns: Vec<Turn<'a>>,
  launches: Vec<Launch<'a>>,
}

impl<'a> Matched<'a> {
  pub(super) fn of(projects: &'a [ProjectFacts], filter: &UsageFilter) -> Self {
    let mut matched = Self {
      turns: Vec::new(),
      launches: Vec::new(),
    };
    for project in projects {
      let runs: HashMap<RunId, &RunFacts> = project.facts.runs.iter().map(|run| (run.id, run)).collect();
      // Every invocation with its run: a review counts by its reviewer, whether its invocation's agent does or not.
      let mut launches: HashMap<InvocationId, (Launch<'a>, bool)> = HashMap::new();
      for invocation in &project.facts.invocations {
        let Some(run) = runs.get(&invocation.run) else {
          continue;
        };
        let launch = Launch {
          project,
          run,
          invocation,
        };
        let admitted = filter.admits(project.project, run, invocation);
        if admitted {
          matched.launches.push(launch);
        }
        launches.insert(invocation.id, (launch, admitted));
      }
      matched.turns.extend(project.facts.turns.iter().filter_map(|turn| {
        let (launch, admitted) = launches.get(&turn.invocation)?;
        admitted.then_some(Turn {
          launch: *launch,
          at: turn.at,
          usage: turn.usage,
          by: Spender::Agent,
        })
      }));
      matched.turns.extend(project.facts.reviews.iter().filter_map(|review| {
        let (launch, _) = launches.get(&review.invocation)?;
        let reviewer = (review.cli, &review.model);
        filter
          .admits_review((project.project, launch.run, launch.invocation), reviewer)
          .then_some(Turn {
            launch: *launch,
            at: review.at,
            usage: review.usage,
            by: Spender::Reviewer {
              cli: review.cli,
              model: &review.model,
            },
          })
      }));
    }
    matched
  }

  /// The turns that completed in `range` and the invocations that ran in it.
  pub(super) fn within(&self, range: DayRange, now: Timestamp) -> Within<'a> {
    let (start, end) = (range.start(), range.end());
    let turns = self
      .turns
      .iter()
      .filter(|turn| range.contains(turn.at))
      .copied()
      .collect();
    let launches = self
      .launches
      .iter()
      .filter_map(|launch| active_between(launch.invocation, (start, end), now).map(|active| (*launch, active)))
      .collect();
    Within {
      turns,
      launches,
      start,
      end,
      now,
    }
  }
}

/// How long `invocation` was active between `start` and `end`; `None` when it did not run then. An
/// invocation's waits for a person are kept as a total, not as times, so one that crosses an edge
/// is counted for its part of the stretch, at most its active time.
pub(super) fn active_between(
  invocation: &InvocationFacts,
  (start, end): (Timestamp, Timestamp),
  now: Timestamp,
) -> Option<u64> {
  let stop = invocation.ended_at.unwrap_or(now).max(invocation.started_at);
  let started_within = start <= invocation.started_at && invocation.started_at < end;
  if !started_within && (invocation.started_at >= start || stop <= start) {
    return None;
  }
  let overlap = stop.min(end).unix_millis() - invocation.started_at.max(start).unix_millis();
  Some(u64::try_from(overlap).unwrap_or(0).min(invocation.active_ms))
}

/// The turns and invocations of one range.
pub(super) struct Within<'a> {
  pub turns: Vec<Turn<'a>>,
  /// Each invocation that ran in the range with its active time in it.
  pub launches: Vec<(Launch<'a>, u64)>,
  pub start: Timestamp,
  pub end: Timestamp,
  /// What invocations that still run are measured to.
  pub now: Timestamp,
}

impl Within<'_> {
  fn has_activity(&self) -> bool {
    !self.turns.is_empty() || !self.launches.is_empty()
  }

  /// The runs with activity in the range, each once.
  pub fn runs(&self) -> Vec<(&ProjectFacts, &RunFacts)> {
    let mut seen: HashMap<(ProjectId, RunId), ()> = HashMap::new();
    let mut runs = Vec::new();
    let all = self
      .launches
      .iter()
      .map(|(launch, _)| (launch.project, launch.run))
      .chain(self.turns.iter().map(|turn| (turn.launch.project, turn.launch.run)));
    for (project, run) in all {
      if seen.insert((project.project, run.id), ()).is_none() {
        runs.push((project, run));
      }
    }
    runs
  }

  pub(super) fn summary(&self) -> Summary {
    let runs = self.runs();
    Summary {
      usage: self.turns.iter().map(|turn| turn.usage).sum(),
      runs: runs.len(),
      finished: runs.iter().filter(|(_, run)| run.outcome.finished()).count(),
      failed: runs.iter().filter(|(_, run)| run.outcome.failed()).count(),
      agent_ms: self.launches.iter().map(|(_, active)| active).sum(),
      failed_spent: self
        .turns
        .iter()
        .filter(|turn| turn.run().outcome.failed())
        .map(|turn| turn.usage.counted())
        .sum(),
    }
  }
}
