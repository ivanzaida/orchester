//! What a profile runs with (`Agent CLI`, `Connection`, `Model`, `Reasoning effort` in A26, L1 and L17): the CLIs this
//! computer has a connection of (and the one an edited profile has), a local profile's connection among those of its
//! CLI, and the models and efforts that connection lists, with the profile's own kept when this computer does not list
//! them. A form reads them from this computer's connections ([`Runs::of`]); a local profile's panel from what the hub
//! says of the profile it holds ([`Runs::held`]), whose choices the workspace makes ([`super::held`]).

mod rows;

use orchester_core::{
  connection::{AgentConnection, AgentProvider, REASONING_EFFORT},
  id::ConnectionId,
  resource::ModelId,
};
use orchester_workspace::agents::{
  form::{effort_options, model_options},
  local_hub::LocalProfile,
  team_profile::ConnectionChoice,
};

use super::{Form, ProfileFields, Scope};

/// The value the CLI select holds for `cli`: an identifier, never shown.
pub(super) fn cli_value(cli: AgentProvider) -> String {
  format!("{cli:?}")
}

/// What the fields come to, with what each select offers.
pub(super) struct Runs {
  clis: Vec<AgentProvider>,
  pub cli: Option<AgentProvider>,
  /// This computer's connections of the CLI chosen.
  of_kind: Vec<ConnectionChoice>,
  /// The connection whose lists the models and efforts are: a local profile's own, a team's the first of its kind.
  pub connection: Option<ConnectionId>,
  models: Vec<String>,
  pub model: Option<ModelId>,
  efforts: Vec<String>,
  /// The effort chosen among those offered; `None` is the CLI's default.
  pub effort: Option<String>,
}

impl Runs {
  /// What the fields come to: from what a local profile's panel holds while the form has one, else from this
  /// computer's connections as the fields choose among them.
  pub(super) fn of(fields: &ProfileFields, form: &Form<'_>) -> Self {
    match form.held {
      Some(held) => Self::held(held),
      None => Self::offered(fields, form),
    }
  }

  fn offered(fields: &ProfileFields, form: &Form<'_>) -> Self {
    let agents: Vec<&AgentConnection> = form
      .offer
      .connections
      .iter()
      .filter(|connection| connection.provider().runs_agents())
      .collect();
    let mut clis: Vec<AgentProvider> = Vec::new();
    for provider in agents
      .iter()
      .map(|connection| connection.provider())
      .chain(form.base.map(|base| base.cli()))
    {
      if !clis.contains(&provider) {
        clis.push(provider);
      }
    }
    let chosen = fields.cli.get();
    let cli = clis.iter().copied().find(|cli| cli_value(*cli) == chosen);
    let of_kind: Vec<&AgentConnection> = agents
      .into_iter()
      .filter(|connection| Some(connection.provider()) == cli)
      .collect();
    let connection = match (form.scope, of_kind.as_slice()) {
      (Scope::Team, kind) => kind.first().copied(),
      (Scope::Local, [only]) => Some(*only),
      (Scope::Local, several) => {
        let chosen = fields.connection.get();
        several
          .iter()
          .find(|connection| connection.id().to_string() == chosen)
          .copied()
      }
    };
    let base = form.base.filter(|base| Some(base.cli()) == cli);
    let mut models: Vec<String> = connection.map_or_else(Vec::new, |connection| {
      model_options(connection, base.map(|base| base.model()))
        .into_iter()
        .map(|option| option.id.as_str().to_owned())
        .collect()
    });
    keep(&mut models, base.map(|base| base.model().as_str().to_owned()));
    let typed = fields.model.get();
    let model = models
      .iter()
      .find(|model| **model == typed)
      .and_then(|model| ModelId::new(model).ok());
    let mut efforts = match (connection, &model) {
      (Some(connection), Some(model)) => effort_options(connection, model),
      _ => Vec::new(),
    };
    let base_effort = base
      .filter(|base| Some(base.model()) == model.as_ref())
      .and_then(|base| {
        base
          .settings()
          .iter()
          .find(|(name, _)| *name == REASONING_EFFORT)
          .map(|(_, value)| value.to_owned())
      });
    keep(&mut efforts, base_effort);
    let typed = fields.effort.get();
    let effort = efforts.iter().find(|effort| **effort == typed).cloned();
    Self {
      clis,
      cli,
      connection: connection.map(|connection| connection.id()),
      of_kind: of_kind
        .into_iter()
        .map(|connection| ConnectionChoice::of(connection))
        .collect(),
      models,
      model,
      efforts,
      effort,
    }
  }

  /// What the local profile `held` runs with, as the hub says of it: its CLI, its connection among those of its CLI,
  /// and what its connection lists, its own model and effort kept.
  fn held(held: &LocalProfile) -> Self {
    let mut models = held.options.models.clone();
    keep(&mut models, Some(held.model.clone()));
    let mut efforts = held.options.efforts.clone();
    keep(&mut efforts, held.effort.clone());
    Self {
      clis: held.options.clis.clone(),
      cli: held.cli,
      of_kind: held.connections.clone(),
      connection: held.current,
      models,
      model: ModelId::new(&held.model).ok(),
      efforts,
      effort: held.effort.clone(),
    }
  }

  /// The connection a local form's fields run on without asking: the only one of its CLI.
  pub(super) fn chosen_for_it(&self) -> Option<ConnectionId> {
    match self.of_kind.as_slice() {
      [only] => Some(only.id),
      _ => None,
    }
  }
}

/// Adds `value` to `list` when it is not in it: what a profile has, though this computer does not list it.
fn keep(list: &mut Vec<String>, value: Option<String>) {
  if let Some(value) = value.filter(|value| !list.contains(value)) {
    list.push(value);
  }
}
