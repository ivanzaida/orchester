//! The dashboard's filters (*Provider*, *Model*, *Agent profile*, *Pipeline*, *Tag*, and *Project* across
//! projects) and the narrower ones a link or a tool asks for (a CLI, a task, a run, a step): each is a set of chosen
//! values, empty for all; a turn counts when every filter admits it. *Tag* matches a run whose task has any chosen
//! tag. An agent profile or a pipeline is chosen by its id or its name in any case, a task by its key or its id.
//! *Provider* chooses agent connections and *Earlier runs*, whose provider no record names; a command tool runs on
//! no provider, so choosing any leaves it out. Plan limits follow the CLI filter only. A review of auto mode's
//! reviewer is matched by the reviewer's CLI, model and provider and by the step whose request it judged; it has no
//! agent profile.

use std::{collections::BTreeMap, sync::Arc};

use super::{
  SpentOn,
  facts::{InvocationFacts, ProjectFacts, RunFacts},
};
use crate::{
  connection::AgentProvider,
  id::{ConnectionId, ProjectId, ResourceId, RunId},
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct UsageFilter {
  pub clis: Vec<AgentProvider>,
  pub models: Vec<Arc<str>>,
  /// Agent profiles by id or name.
  pub profiles: Vec<Arc<str>>,
  /// Pipelines by id or name.
  pub pipelines: Vec<Arc<str>>,
  pub tags: Vec<Arc<str>>,
  pub projects: Vec<ProjectId>,
  /// Tasks by key (`KON-12`, any case) or id.
  pub tasks: Vec<Arc<str>>,
  pub runs: Vec<RunId>,
  /// Steps by name, in whichever pipeline.
  pub steps: Vec<Arc<str>>,
  /// Agent providers, by their connection.
  pub providers: Vec<ConnectionId>,
  /// Usage whose provider no record names is chosen too (*Earlier runs* in the Provider filter).
  pub earlier: bool,
}

impl UsageFilter {
  pub fn is_empty(&self) -> bool {
    self == &Self::default()
  }

  /// Whether `invocation` of `run` in `project` counts.
  pub fn admits(&self, project: ProjectId, run: &RunFacts, invocation: &InvocationFacts) -> bool {
    // A command tool runs no model, and so on no provider.
    let on = invocation.model.is_some().then(|| SpentOn::of(invocation.connection));
    self.admits_run(project, run)
      && chosen(&self.clis, invocation.cli.as_ref())
      && chosen(&self.models, invocation.model.as_ref())
      && named(&self.profiles, invocation.profile_id, invocation.profile.as_deref())
      && chosen(&self.steps, Some(&invocation.step))
      && self.admits_provider(on)
  }

  /// Whether a review by auto mode's reviewer on `cli` with `model` spent `on` a provider, of a request `invocation`
  /// of `run` in `project` made, counts.
  pub fn admits_review(
    &self,
    (project, run, invocation): (ProjectId, &RunFacts, &InvocationFacts),
    (cli, model, on): (AgentProvider, &Arc<str>, SpentOn),
  ) -> bool {
    self.admits_run(project, run)
      && chosen(&self.clis, Some(&cli))
      && chosen(&self.models, Some(model))
      && chosen(&self.profiles, None)
      && chosen(&self.steps, Some(&invocation.step))
      && self.admits_provider(Some(on))
  }

  /// Whether usage spent `on` a provider (`None`: on none, a command tool) counts: everything while no provider is
  /// chosen, else the chosen providers and, when *Earlier runs* is chosen, usage whose provider no record names.
  pub fn admits_provider(&self, on: Option<SpentOn>) -> bool {
    if !self.chooses_providers() {
      return true;
    }
    match on {
      Some(SpentOn::Provider(connection)) => self.providers.contains(&connection),
      Some(SpentOn::Earlier) => self.earlier,
      None => false,
    }
  }

  /// Whether it chooses providers or *Earlier runs*.
  pub fn chooses_providers(&self) -> bool {
    !self.providers.is_empty() || self.earlier
  }

  fn admits_run(&self, project: ProjectId, run: &RunFacts) -> bool {
    chosen(&self.projects, Some(&project))
      && named(&self.pipelines, run.pipeline_id, run.pipeline.as_deref())
      && (self.tags.is_empty() || run.tags.iter().any(|tag| self.tags.contains(tag)))
      && chosen(&self.runs, Some(&run.id))
      && (self.tasks.is_empty() || self.tasks.iter().any(|task| names_task(task, run)))
  }

  /// Whether the plan limits of `cli` are shown.
  pub fn admits_cli(&self, cli: AgentProvider) -> bool {
    self.clis.is_empty() || self.clis.contains(&cli)
  }

  /// Whether it chooses whose plan limits are shown: CLIs.
  pub fn chooses_limits(&self) -> bool {
    !self.clis.is_empty()
  }
}

/// A filter admits what it has chosen, or everything while nothing is chosen; a value the record
/// does not have is admitted only then.
fn chosen<T: PartialEq>(values: &[T], value: Option<&T>) -> bool {
  values.is_empty() || value.is_some_and(|value| values.contains(value))
}

/// [`chosen`] for a resource chosen by its id or its name, in any case.
fn named(values: &[Arc<str>], id: Option<ResourceId>, name: Option<&str>) -> bool {
  let id = id.map(|id| id.to_string());
  let name = name.map(str::to_lowercase);
  values.is_empty()
    || values
      .iter()
      .any(|value| name.as_deref() == Some(&*value.to_lowercase()) || id.as_deref() == Some(&**value))
}

/// Whether `value` names the task of `run`: its key, in any case, or its id.
fn names_task(value: &str, run: &RunFacts) -> bool {
  run.task_key.eq_ignore_ascii_case(value) || run.task.to_string() == value
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
      for review in &project.facts.reviews {
        if !options.clis.contains(&review.cli) {
          options.clis.push(review.cli);
        }
        add(&mut options.models, Some(&review.model));
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
