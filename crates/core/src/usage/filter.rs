//! The dashboard's filters (*CLI*, *Model*, *Agent profile*, *Pipeline*, *Tag*, and *Project* across
//! projects): each is a set of chosen values, empty for all; a turn counts when every filter admits
//! it. *Tag* matches a run whose task has any chosen tag. Plan limits follow the CLI filter only.

use std::{collections::BTreeMap, sync::Arc};

use super::facts::{InvocationFacts, ProjectFacts, RunFacts};
use crate::{connection::AgentProvider, id::ProjectId};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsageFilter {
  pub clis: Vec<AgentProvider>,
  pub models: Vec<Arc<str>>,
  pub profiles: Vec<Arc<str>>,
  pub pipelines: Vec<Arc<str>>,
  pub tags: Vec<Arc<str>>,
  pub projects: Vec<ProjectId>,
}

impl UsageFilter {
  pub fn is_empty(&self) -> bool {
    self.clis.is_empty()
      && self.models.is_empty()
      && self.profiles.is_empty()
      && self.pipelines.is_empty()
      && self.tags.is_empty()
      && self.projects.is_empty()
  }

  /// Whether `invocation` of `run` in `project` counts.
  pub fn admits(&self, project: ProjectId, run: &RunFacts, invocation: &InvocationFacts) -> bool {
    chosen(&self.projects, Some(&project))
      && chosen(&self.clis, invocation.cli.as_ref())
      && chosen(&self.models, invocation.model.as_ref())
      && chosen(&self.profiles, invocation.profile.as_ref())
      && chosen(&self.pipelines, run.pipeline.as_ref())
      && (self.tags.is_empty() || run.tags.iter().any(|tag| self.tags.contains(tag)))
  }

  /// Whether the plan limits of `cli` are shown.
  pub fn admits_cli(&self, cli: AgentProvider) -> bool {
    self.clis.is_empty() || self.clis.contains(&cli)
  }
}

/// A filter admits what it has chosen, or everything while nothing is chosen; a value the record
/// does not have is admitted only then.
fn chosen<T: PartialEq>(values: &[T], value: Option<&T>) -> bool {
  values.is_empty() || value.is_some_and(|value| values.contains(value))
}

/// Adds `value` to the chosen values, or removes it when it was chosen.
pub fn toggle<T: PartialEq>(values: &mut Vec<T>, value: T) {
  if let Some(position) = values.iter().position(|chosen| *chosen == value) {
    values.remove(position);
  } else {
    values.push(value);
  }
}

/// What each filter menu offers: the values the projects' records have, sorted by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FilterOptions {
  pub clis: Vec<AgentProvider>,
  pub models: Vec<Arc<str>>,
  pub profiles: Vec<Arc<str>>,
  pub pipelines: Vec<Arc<str>>,
  /// Each tag with how many runs carry it.
  pub tags: Vec<(Arc<str>, usize)>,
  pub projects: Vec<(ProjectId, Arc<str>)>,
}

impl FilterOptions {
  pub fn of(projects: &[ProjectFacts]) -> Self {
    let mut options = Self::default();
    let mut tags: BTreeMap<Arc<str>, usize> = BTreeMap::new();
    for project in projects {
      options.projects.push((project.project, project.name.clone()));
      for invocation in &project.facts.invocations {
        if let Some(cli) = invocation.cli
          && !options.clis.contains(&cli)
        {
          options.clis.push(cli);
        }
        add(&mut options.models, invocation.model.as_ref());
        add(&mut options.profiles, invocation.profile.as_ref());
      }
      for run in &project.facts.runs {
        add(&mut options.pipelines, run.pipeline.as_ref());
        for tag in &run.tags {
          *tags.entry(tag.clone()).or_default() += 1;
        }
      }
    }
    options.clis.sort_by_key(|cli| super::CliSeries::of(Some(*cli)));
    options.models.sort();
    options.profiles.sort();
    options.pipelines.sort();
    options.tags = tags.into_iter().collect();
    options.projects.sort_by(|a, b| a.1.cmp(&b.1));
    options
  }
}

fn add(values: &mut Vec<Arc<str>>, value: Option<&Arc<str>>) {
  if let Some(value) = value
    && !values.contains(value)
  {
    values.push(value.clone());
  }
}
