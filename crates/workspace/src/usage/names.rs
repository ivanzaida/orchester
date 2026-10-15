//! What the ids a filter or a link holds stand for: an agent profile's or a pipeline's name, a run's task
//! and number, an account. A filter chooses a profile or a pipeline by its id or its name, so a value is
//! resolved through what the read usage names; a value that names nothing known stays as it was given.

use std::sync::Arc;

use orchester_core::{
  account::AgentAccount,
  id::{AgentAccountId, ResourceId, RunId},
  run::RunNumber,
};

use super::UsageData;

/// Each known resource with its name, sorted by name, then id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Names {
  pub profiles: Vec<(ResourceId, Arc<str>)>,
  pub pipelines: Vec<(ResourceId, Arc<str>)>,
  /// Each run with its task's key and its number.
  pub runs: Vec<(RunId, Arc<str>, RunNumber)>,
  pub accounts: Vec<AgentAccount>,
}

impl Names {
  pub fn of(data: &UsageData) -> Self {
    let mut names = Self {
      accounts: data.accounts.to_vec(),
      ..Self::default()
    };
    for project in data.projects.iter() {
      for invocation in &project.facts.invocations {
        if let (Some(id), Some(name)) = (invocation.profile_id, &invocation.profile) {
          names.profiles.push((id, name.clone()));
        }
      }
      for run in &project.facts.runs {
        if let (Some(id), Some(name)) = (run.pipeline_id, &run.pipeline) {
          names.pipelines.push((id, name.clone()));
        }
        names.runs.push((run.id, run.task_key.clone(), run.number));
      }
    }
    for list in [&mut names.profiles, &mut names.pipelines] {
      list.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
      list.dedup();
    }
    names
  }

  /// Whether `value`, a resource's id or name, names `(id, name)`.
  pub fn names(value: &str, (id, name): (ResourceId, &str)) -> bool {
    value == name || value == id.to_string()
  }

  /// The name of the resource `value` names among `list`, or `None` when it names none of them.
  pub fn resolve<'a>(list: &'a [(ResourceId, Arc<str>)], value: &str) -> Option<&'a Arc<str>> {
    list
      .iter()
      .find(|(id, name)| Self::names(value, (*id, name)))
      .map(|(_, name)| name)
  }

  pub fn run(&self, run: RunId) -> Option<(&Arc<str>, RunNumber)> {
    self
      .runs
      .iter()
      .find(|(id, ..)| *id == run)
      .map(|(_, key, number)| (key, *number))
  }

  pub fn account(&self, account: AgentAccountId) -> Option<&AgentAccount> {
    self.accounts.iter().find(|known| known.id() == account)
  }
}
