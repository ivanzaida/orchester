//! *Where tokens go*: spent tokens by agent profile · step, by model, by pipeline and, across
//! projects, by project; each row split by CLI. Auto mode's reviewer is no agent profile: its reviews
//! have a row per model of their own among the profiles (*Auto mode reviewer · claude-haiku-4-5*).

use std::{collections::HashMap, sync::Arc};

use super::{
  AccountUsage, CliSeries,
  facts::ProjectFacts,
  report::{Spender, Within},
};
use crate::{
  id::{AgentAccountId, ProjectId},
  run::TokenUsage,
};

/// What a row groups by.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BreakdownKey {
  ProfileStep {
    profile: Arc<str>,
    step: Arc<str>,
  },
  /// Auto mode's reviewer with this model, in the profiles' column.
  Reviewer(Arc<str>),
  Model(Arc<str>),
  /// `None` for runs whose pipeline is gone.
  Pipeline(Option<Arc<str>>),
  Project(ProjectId, Arc<str>),
}

/// One row: spent tokens per CLI (indexed by [`CliSeries::index`]), and its usage per account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BreakdownRow {
  pub key: BreakdownKey,
  pub by_cli: [u64; 3],
  pub by_account: AccountUsage,
}

impl BreakdownRow {
  pub fn total(&self) -> u64 {
    self.by_cli.iter().sum()
  }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Breakdowns {
  pub profiles: Vec<BreakdownRow>,
  pub models: Vec<BreakdownRow>,
  pub pipelines: Vec<BreakdownRow>,
  pub projects: Vec<BreakdownRow>,
  /// Listed projects whose runs spent nothing in the range (*Atlas had no runs in the period.*).
  pub idle_projects: Vec<Arc<str>>,
}

impl Breakdowns {
  pub(super) fn of(within: &Within<'_>, projects: &[ProjectFacts]) -> Self {
    let mut profiles = Rows::default();
    let mut models = Rows::default();
    let mut pipelines = Rows::default();
    let mut by_project = Rows::default();
    for turn in &within.turns {
      let invocation = turn.launch.invocation;
      let series = (CliSeries::of(turn.cli()), turn.account(), turn.usage);
      let profile = match turn.by {
        Spender::Agent => invocation.profile.as_ref().map(|profile| BreakdownKey::ProfileStep {
          profile: profile.clone(),
          step: invocation.step.clone(),
        }),
        Spender::Reviewer { model, .. } => Some(BreakdownKey::Reviewer(model.clone())),
      };
      if let Some(key) = profile {
        profiles.add(key, series);
      }
      if let Some(model) = turn.model() {
        models.add(BreakdownKey::Model(model.clone()), series);
      }
      pipelines.add(BreakdownKey::Pipeline(turn.run().pipeline.clone()), series);
      let project = turn.launch.project;
      by_project.add(BreakdownKey::Project(project.project, project.name.clone()), series);
    }
    let projects_rows = by_project.sorted();
    let idle_projects = projects
      .iter()
      .filter(|project| {
        !projects_rows
          .iter()
          .any(|row| matches!(&row.key, BreakdownKey::Project(id, _) if *id == project.project))
      })
      .map(|project| project.name.clone())
      .collect();
    Self {
      profiles: profiles.sorted(),
      models: models.sorted(),
      pipelines: pipelines.sorted(),
      projects: projects_rows,
      idle_projects,
    }
  }
}

#[derive(Default)]
struct Rows(HashMap<BreakdownKey, ([u64; 3], AccountUsage)>);

impl Rows {
  /// A turn's tokens under `key`, on its CLI and its account.
  fn add(&mut self, key: BreakdownKey, (cli, account, usage): (CliSeries, Option<AgentAccountId>, TokenUsage)) {
    let (by_cli, by_account) = self.0.entry(key).or_default();
    by_cli[cli.index()] = by_cli[cli.index()].saturating_add(usage.counted());
    by_account.add(account, usage);
  }

  /// Rows with tokens, most first; ties by key so the order is stable.
  fn sorted(self) -> Vec<BreakdownRow> {
    let mut rows: Vec<BreakdownRow> = self
      .0
      .into_iter()
      .map(|(key, (by_cli, by_account))| BreakdownRow {
        key,
        by_cli,
        by_account,
      })
      .filter(|row| row.total() > 0)
      .collect();
    rows.sort_by(|a, b| {
      b.total()
        .cmp(&a.total())
        .then_with(|| label(&a.key).cmp(&label(&b.key)))
    });
    rows
  }
}

fn label(key: &BreakdownKey) -> (&str, &str) {
  match key {
    BreakdownKey::ProfileStep { profile, step } => (profile, step),
    BreakdownKey::Reviewer(model) => ("", model),
    BreakdownKey::Model(name) | BreakdownKey::Project(_, name) => (name, ""),
    BreakdownKey::Pipeline(name) => (name.as_deref().unwrap_or(""), ""),
  }
}
