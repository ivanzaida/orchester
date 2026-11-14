//! What a profile whose CLI and model are decided can run on on this computer (`Agents on accounts` C3 `OTVQt`): this
//! computer's accounts of the CLI, each once, whether the project has a connection on one or not, then the project's
//! connections on a key. A team profile's *Runs on* select offers them, and the fix of a profile that runs on nothing
//! takes the first. One that lists models, the profile's not among them, is offered but cannot be chosen: the model
//! is the profile's, not this computer's.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{connection::AgentProvider, id::ConnectionId};
use orchester_workspace::agents::runs_on::RunsOnChoice;

use super::{
  Here,
  runs::picker::{self, Item, Target},
};
use crate::{
  screens::team_hub::words::cli_key,
  services::{
    accounts::{self, plan_name},
    plan_limits,
  },
};

/// One thing the profile can run on here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens) struct Offered {
  /// What a select holds for it: the project's connection on it, by id, else the account's own item.
  pub value: String,
  /// *Claude Code · Work*.
  pub label: Arc<str>,
  /// An account's plan (*Max plan*), or that it lists other models.
  pub detail: Option<Arc<str>>,
  /// It lists models and the profile's is not one of them: it cannot be chosen.
  pub other_models: bool,
  /// What choosing it makes the profile run on.
  pub here: Here,
}

/// What a profile of `cli` with `model` can run on here among `runs_on`, the project's connections of that CLI, the
/// one it runs on now being `current`.
pub(in crate::screens) fn offered(
  ctx: &mut Ctx,
  (cli, model): (AgentProvider, &str),
  (runs_on, current): (&[RunsOnChoice], Option<ConnectionId>),
) -> Vec<Offered> {
  let accounts = accounts::listed(ctx);
  let readings = plan_limits::account_readings(ctx);
  let api = accounts::api_listed(ctx);
  let who = picker::Who {
    accounts: &accounts,
    readings: &readings,
    api: &api.0,
    connecting: None,
  };
  let cli_words = ctx.t(cli_key(cli));
  picker::items(runs_on, current, &who)
    .iter()
    .filter(|item| item.target.cli() == cli)
    .filter_map(|item| one(ctx, item, (&cli_words, model), &who))
    .collect()
}

/// What the value a select holds for an [`Offered`] makes the profile run on: a connection's id, or an account's
/// item; `None` for anything else.
pub(in crate::screens) fn here_of(value: &str) -> Option<Here> {
  match value.strip_prefix("account-") {
    Some(account) => account.parse().ok().map(Here::Account),
    None => value.parse().ok().map(Here::Connection),
  }
}

/// The first of `offered` that can be chosen: what a fix takes when the person chose nothing.
pub(in crate::screens) fn first(offered: &[Offered]) -> Option<&Offered> {
  offered.iter().find(|offered| !offered.other_models)
}

fn one(ctx: &Ctx, item: &Item, (cli, model): (&str, &str), who: &picker::Who<'_>) -> Option<Offered> {
  let connection = item.target.connection();
  let here = match (connection, &item.target) {
    (Some(choice), _) => Here::Connection(choice.id),
    (None, Target::Account(account)) => Here::Account(account.id),
    // An API account is offered once the project has its connection on it, which the picker makes; a CLI is a
    // team form's choice, never something to run on.
    (None, _) => return None,
  };
  let other_models = connection.is_some_and(|choice| !choice.models.is_empty() && !choice.serves(model));
  let plan = match &item.target {
    Target::Account(account) => who
      .accounts
      .of(account.id)
      .and_then(|account| account.last_check())
      .and_then(|check| check.known_identity())
      .and_then(|identity| identity.plan.as_deref())
      .map(|plan| ctx.t_args("team_hub.runsHere.plan", [("plan", plan_name(plan))])),
    _ => None,
  };
  Some(Offered {
    value: connection.map_or_else(|| item.id.to_string(), |choice| choice.id.to_string()),
    label: ctx.t_args(
      "team_hub.profile.connectionName",
      [("cli", cli), ("name", &picker::name_of(ctx, &item.target, who))],
    ),
    detail: if other_models {
      Some(ctx.t("team_hub.runsHere.otherModels"))
    } else {
      plan
    },
    other_models,
    here,
  })
}
