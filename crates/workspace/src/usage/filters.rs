//! The page's filters as a person works them (`et35n`, *Usage · interactions*): the five menus (*CLI*,
//! *Model*, *Agent profile*, *Pipeline*, *Tag*) with what each offers and which are chosen, the chips of the
//! chosen values, the narrower filters a link or a tool may add (a task, a run, a step, an account), and a
//! *Where tokens go* row chosen as a filter. A menu chooses a profile or a pipeline by its id, so a link
//! keeps naming it after a rename; a value chosen by name, from a row or a link, counts as chosen too.

use std::sync::Arc;

use orchester_core::{
  account::AccountCli,
  connection::AgentProvider,
  id::{AgentAccountId, ResourceId, RunId},
  run::RunNumber,
  usage::{BreakdownKey, UsageFilter},
};

use super::{Dashboard, UsageQuery, names::Names};

/// One filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Facet {
  Cli,
  Model,
  Profile,
  Pipeline,
  Tag,
  Task,
  Run,
  Step,
  Account,
}

impl Facet {
  /// The filters the toolbar has a menu for, in its order.
  pub const MENUS: [Self; 5] = [Self::Cli, Self::Model, Self::Profile, Self::Pipeline, Self::Tag];
  /// Every filter, in the order their chips read.
  pub const ALL: [Self; 9] = [
    Self::Cli,
    Self::Model,
    Self::Profile,
    Self::Pipeline,
    Self::Tag,
    Self::Task,
    Self::Run,
    Self::Step,
    Self::Account,
  ];
}

/// One value of a filter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FilterValue {
  Cli(AgentProvider),
  /// A model, profile, pipeline, tag, task or step, by id or name.
  Text(Arc<str>),
  Run(RunId),
  Account(AgentAccountId),
}

/// How a value reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueLabel {
  Cli(AgentProvider),
  Name(Arc<str>),
  /// `KON-12 · Run 3`.
  Run {
    task: Arc<str>,
    number: RunNumber,
  },
  /// An account by its name; `None` for a CLI's Default nobody renamed, which reads as *Default*.
  Account {
    cli: AccountCli,
    name: Option<Arc<str>>,
  },
  /// An id that names nothing the page read, shown as it was given.
  Unknown(Arc<str>),
}

/// What a menu offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterOption {
  pub value: FilterValue,
  pub label: ValueLabel,
  pub chosen: bool,
  /// How many runs carry a tag.
  pub runs: Option<usize>,
}

impl Dashboard {
  /// What `facet`'s menu offers: the values the period's usage has, sorted by name.
  pub fn options(&self, facet: Facet) -> Vec<FilterOption> {
    let filter = &self.query.filter;
    let option = |value: FilterValue, label: ValueLabel, chosen: bool| FilterOption {
      value,
      label,
      chosen,
      runs: None,
    };
    let texts = |values: &[Arc<str>], chosen: &[Arc<str>]| -> Vec<FilterOption> {
      values
        .iter()
        .map(|value| {
          let chosen = chosen.contains(value);
          option(
            FilterValue::Text(value.clone()),
            ValueLabel::Name(value.clone()),
            chosen,
          )
        })
        .collect()
    };
    let resources = |list: &[(ResourceId, Arc<str>)], chosen: &[Arc<str>]| -> Vec<FilterOption> {
      list
        .iter()
        .map(|(id, name)| {
          let chosen = chosen.iter().any(|value| Names::names(value, (*id, name)));
          option(
            FilterValue::Text(id.to_string().into()),
            ValueLabel::Name(name.clone()),
            chosen,
          )
        })
        .collect()
    };
    match facet {
      Facet::Cli => self
        .options
        .clis
        .iter()
        .map(|cli| option(FilterValue::Cli(*cli), ValueLabel::Cli(*cli), filter.clis.contains(cli)))
        .collect(),
      Facet::Model => texts(&self.options.models, &filter.models),
      Facet::Profile => resources(&self.names.profiles, &filter.profiles),
      Facet::Pipeline => resources(&self.names.pipelines, &filter.pipelines),
      Facet::Tag => self
        .options
        .tags
        .iter()
        .map(|(tag, runs)| FilterOption {
          runs: Some(*runs),
          ..option(
            FilterValue::Text(tag.clone()),
            ValueLabel::Name(tag.clone()),
            filter.tags.contains(tag),
          )
        })
        .collect(),
      Facet::Task | Facet::Run | Facet::Step | Facet::Account => Vec::new(),
    }
  }

  /// The chosen values of `facet` with how each reads, in the order they were chosen.
  pub fn chosen(&self, facet: Facet) -> Vec<(FilterValue, ValueLabel)> {
    let filter = &self.query.filter;
    let texts = |values: &[Arc<str>], label: &dyn Fn(&Arc<str>) -> ValueLabel| -> Vec<(FilterValue, ValueLabel)> {
      values
        .iter()
        .map(|value| (FilterValue::Text(value.clone()), label(value)))
        .collect()
    };
    let profile = |value: &Arc<str>| resolved(&self.names.profiles, value);
    let pipeline = |value: &Arc<str>| resolved(&self.names.pipelines, value);
    let plain = |value: &Arc<str>| ValueLabel::Name(value.clone());
    match facet {
      Facet::Cli => filter
        .clis
        .iter()
        .map(|cli| (FilterValue::Cli(*cli), ValueLabel::Cli(*cli)))
        .collect(),
      Facet::Model => texts(&filter.models, &plain),
      Facet::Profile => texts(&filter.profiles, &profile),
      Facet::Pipeline => texts(&filter.pipelines, &pipeline),
      Facet::Tag => texts(&filter.tags, &plain),
      Facet::Task => texts(&filter.tasks, &plain),
      Facet::Step => texts(&filter.steps, &plain),
      Facet::Run => filter
        .runs
        .iter()
        .map(|run| {
          let label = self.names.run(*run).map_or_else(
            || ValueLabel::Unknown(run.to_string().into()),
            |(task, number)| ValueLabel::Run {
              task: task.clone(),
              number,
            },
          );
          (FilterValue::Run(*run), label)
        })
        .collect(),
      Facet::Account => filter
        .accounts
        .iter()
        .map(|account| {
          let label = self.names.account(*account).map_or_else(
            || ValueLabel::Unknown(account.to_string().into()),
            |known| ValueLabel::Account {
              cli: known.cli(),
              name: known.name().map(|name| name.as_str().into()),
            },
          );
          (FilterValue::Account(*account), label)
        })
        .collect(),
    }
  }
}

impl Dashboard {
  /// The query with `value` of `facet` chosen, or no longer chosen when it was: a profile or pipeline
  /// chosen by its id or its name is let go either way.
  pub fn toggled(&self, facet: Facet, value: &FilterValue) -> UsageQuery {
    let mut filter = self.query.filter.clone();
    if !self.remove_value(&mut filter, facet, value) {
      add_value(&mut filter, facet, value.clone());
    }
    self.query.with_filter(filter)
  }

  /// The query without `value` of `facet`.
  pub fn without(&self, facet: Facet, value: &FilterValue) -> UsageQuery {
    let mut filter = self.query.filter.clone();
    self.remove_value(&mut filter, facet, value);
    self.query.with_filter(filter)
  }

  /// The query with nothing of `facet` chosen.
  pub fn cleared(&self, facet: Facet) -> UsageQuery {
    let mut filter = self.query.filter.clone();
    match facet {
      Facet::Cli => filter.clis.clear(),
      Facet::Model => filter.models.clear(),
      Facet::Profile => filter.profiles.clear(),
      Facet::Pipeline => filter.pipelines.clear(),
      Facet::Tag => filter.tags.clear(),
      Facet::Task => filter.tasks.clear(),
      Facet::Run => filter.runs.clear(),
      Facet::Step => filter.steps.clear(),
      Facet::Account => filter.accounts.clear(),
    }
    self.query.with_filter(filter)
  }

  /// The query without any filter (*Clear filters*).
  pub fn unfiltered(&self) -> UsageQuery {
    self.query.with_filter(UsageFilter::default())
  }

  /// The query with a *Where tokens go* row's profile, model or pipeline chosen (the row of a profile ·
  /// step chooses its profile); `None` for a row that names nothing to choose: auto mode's reviewer, or
  /// runs whose pipeline is gone.
  pub fn with_row(&self, key: &BreakdownKey) -> Option<UsageQuery> {
    let mut filter = self.query.filter.clone();
    let (values, value) = match key {
      BreakdownKey::ProfileStep { profile, .. } => (&mut filter.profiles, profile),
      BreakdownKey::Model(model) => (&mut filter.models, model),
      BreakdownKey::Pipeline(Some(pipeline)) => (&mut filter.pipelines, &pipeline.name),
      BreakdownKey::Pipeline(None) | BreakdownKey::Reviewer(_) | BreakdownKey::Project(..) => return None,
    };
    if !values.contains(value) {
      values.push(value.clone());
    }
    Some(self.query.with_filter(filter))
  }

  /// Removes from `filter` every chosen value that names `value`; whether there was one.
  fn remove_value(&self, filter: &mut UsageFilter, facet: Facet, value: &FilterValue) -> bool {
    let before = filter.clone();
    match value {
      FilterValue::Cli(cli) => filter.clis.retain(|chosen| chosen != cli),
      FilterValue::Run(run) => filter.runs.retain(|chosen| chosen != run),
      FilterValue::Account(account) => filter.accounts.retain(|chosen| chosen != account),
      FilterValue::Text(text) => {
        let list = match facet {
          Facet::Profile => Some(&self.names.profiles),
          Facet::Pipeline => Some(&self.names.pipelines),
          _ => None,
        };
        // The resource `text` names, so that its id and its name are both let go.
        let resource = list.and_then(|list| list.iter().find(|(id, name)| Names::names(text, (*id, name))));
        if let Some(values) = texts_of(filter, facet) {
          values
            .retain(|chosen| chosen != text && resource.is_none_or(|(id, name)| !Names::names(chosen, (*id, name))));
        }
      }
    }
    *filter != before
  }
}

/// The name of the profile or pipeline `value` names among `list`, or `value` as it was given.
fn resolved(list: &[(ResourceId, Arc<str>)], value: &Arc<str>) -> ValueLabel {
  ValueLabel::Name(Names::resolve(list, value).unwrap_or(value).clone())
}

fn texts_of(filter: &mut UsageFilter, facet: Facet) -> Option<&mut Vec<Arc<str>>> {
  match facet {
    Facet::Model => Some(&mut filter.models),
    Facet::Profile => Some(&mut filter.profiles),
    Facet::Pipeline => Some(&mut filter.pipelines),
    Facet::Tag => Some(&mut filter.tags),
    Facet::Task => Some(&mut filter.tasks),
    Facet::Step => Some(&mut filter.steps),
    Facet::Cli | Facet::Run | Facet::Account => None,
  }
}

fn add_value(filter: &mut UsageFilter, facet: Facet, value: FilterValue) {
  match value {
    FilterValue::Cli(cli) => filter.clis.push(cli),
    FilterValue::Run(run) => filter.runs.push(run),
    FilterValue::Account(account) => filter.accounts.push(account),
    FilterValue::Text(text) => {
      if let Some(values) = texts_of(filter, facet) {
        values.push(text);
      }
    }
  }
}
