//! What a profile runs with (`Agent CLI`, `Connection`, `Model`, `Reasoning effort` in A26 and L17): the CLIs this
//! computer has a connection of (and the one an edited profile has), a local profile's connection among those of its
//! CLI (chosen for it when there is one), and the models and efforts that connection lists, with the edited version's
//! own kept when this computer does not list them.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, SelectOption},
  core::Signal,
};
use orchester_core::{
  connection::{AgentConnection, AgentProvider, REASONING_EFFORT},
  resource::ModelId,
};
use orchester_workspace::agents::form::{effort_options, model_options};

use super::{Form, ProfileFields, Scope};
use crate::{
  screens::team_hub::{
    profile::from_team::effort_words,
    rows::{self, Sub},
    words::cli_key,
  },
  ui::field::SelectField,
};

/// The value the CLI select holds for `cli`: an identifier, never shown.
pub(super) fn cli_value(cli: AgentProvider) -> String {
  format!("{cli:?}")
}

/// What the fields come to, with what each select offers.
pub(super) struct Runs {
  clis: Vec<AgentProvider>,
  pub cli: Option<AgentProvider>,
  /// This computer's connections of the CLI chosen.
  of_kind: Vec<AgentConnection>,
  /// The connection whose lists the models and efforts are: a local profile's own, a team's the first of its kind.
  pub connection: Option<AgentConnection>,
  models: Vec<String>,
  pub model: Option<ModelId>,
  efforts: Vec<String>,
  /// The effort chosen among those offered; `None` is the CLI's default.
  pub effort: Option<String>,
}

impl Runs {
  pub(super) fn of(fields: &ProfileFields, form: &Form<'_>) -> Self {
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
    let of_kind: Vec<AgentConnection> = agents
      .iter()
      .filter(|connection| Some(connection.provider()) == cli)
      .map(|connection| (*connection).clone())
      .collect();
    let connection = match (form.scope, of_kind.as_slice()) {
      (Scope::Team, kind) => kind.first().cloned(),
      (Scope::Local, [only]) => Some(only.clone()),
      (Scope::Local, several) => {
        let chosen = fields.connection.get();
        several
          .iter()
          .find(|connection| connection.id().to_string() == chosen)
          .cloned()
      }
    };
    let base = form.base.filter(|base| Some(base.cli()) == cli);
    let mut models: Vec<String> = connection.as_ref().map_or_else(Vec::new, |connection| {
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
    let mut efforts = match (&connection, &model) {
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
      of_kind,
      connection,
      models,
      model,
      efforts,
      effort,
    }
  }

  /// The rows: the CLI, a local profile's connection, the model and the effort.
  pub(super) fn rows(&self, ctx: &mut Ctx, fields: &ProfileFields, form: &Form<'_>) -> Vec<Row> {
    let mut rows = vec![self.cli_row(ctx, fields, form)];
    if form.scope == Scope::Local {
      rows.push(self.connection_row(ctx, fields, form));
    }
    rows.push(self.model_row(ctx, fields, form));
    rows.push(self.effort_row(ctx, fields, form));
    rows
  }

  fn cli_row(&self, ctx: &mut Ctx, fields: &ProfileFields, form: &Form<'_>) -> Row {
    let options = self
      .clis
      .iter()
      .map(|cli| (cli_value(*cli), ctx.t(cli_key(*cli)).to_string()))
      .collect();
    // A team's CLI is a kind, never a connection, which its line says; a local profile names its connection apart.
    let quiet = (form.scope == Scope::Team).then(|| ctx.t("team_hub.profile.agentCliSub"));
    let missing = form.tried && self.cli.is_none();
    let spec = Spec {
      id: "cli",
      label: "team_hub.profile.agentCli",
      placeholder: "team_new.profile.cliPlaceholder",
    };
    select_row(ctx, form, (spec, &fields.cli, options), (missing, quiet, false))
  }

  /// *Connection* of a local profile: chosen for it when its CLI has one, else asked from those of its kind.
  fn connection_row(&self, ctx: &mut Ctx, fields: &ProfileFields, form: &Form<'_>) -> Row {
    let asked = self.of_kind.len() > 1;
    let missing = form.tried && asked && self.connection.is_none();
    let options = self
      .of_kind
      .iter()
      .map(|connection| (connection.id().to_string(), connection.name().as_str().to_owned()))
      .collect();
    let placeholder = match (self.cli.is_some(), asked) {
      (true, true) => "local_new.profile.connectionAsk",
      (true, false) => "local_new.profile.connectionOnly",
      (false, _) => "local_new.profile.connectionAuto",
    };
    let spec = Spec {
      id: "connection",
      label: "local_new.profile.connection",
      placeholder,
    };
    let quiet = Some(ctx.t("local_new.profile.connectionSub"));
    select_row(ctx, form, (spec, &fields.connection, options), (missing, quiet, !asked))
  }

  fn model_row(&self, ctx: &mut Ctx, fields: &ProfileFields, form: &Form<'_>) -> Row {
    let options = self.models.iter().map(|model| (model.clone(), model.clone())).collect();
    let spec = Spec {
      id: "model",
      label: "team_hub.profile.model",
      placeholder: "team_new.profile.modelPlaceholder",
    };
    let missing = form.tried && self.model.is_none();
    select_row(ctx, form, (spec, &fields.model, options), (missing, None, false))
  }

  fn effort_row(&self, ctx: &mut Ctx, fields: &ProfileFields, form: &Form<'_>) -> Row {
    let mut options = vec![(String::new(), ctx.t("team_new.profile.effortDefault").to_string())];
    options.extend(
      self
        .efforts
        .iter()
        .map(|effort| (effort.clone(), effort_words(ctx, effort))),
    );
    let spec = Spec {
      id: "effort",
      label: "team_hub.profile.effort",
      placeholder: "team_new.profile.effortDefault",
    };
    select_row(ctx, form, (spec, &fields.effort, options), (false, None, false))
  }
}

/// Adds `value` to `list` when it is not in it: what an edited version has, though this computer does not list it.
fn keep(list: &mut Vec<String>, value: Option<String>) {
  if let Some(value) = value.filter(|value| !list.contains(value)) {
    list.push(value);
  }
}

/// A select's row: its id after the form's, and the keys of its label and of what it says while nothing is chosen.
struct Spec {
  id: &'static str,
  label: &'static str,
  placeholder: &'static str,
}

/// The row of a select of `options` (value, words) holding `value`: its select is `<rows>-<id>`, the row
/// `<rows>-<id>-row`. `missing` says it is required in `danger` in place of `quiet`; `fixed` keeps it as it is.
fn select_row(
  ctx: &mut Ctx,
  form: &Form<'_>,
  (spec, value, options): (Spec, &Signal<String>, Vec<(String, String)>),
  (missing, quiet, fixed): (bool, Option<Arc<str>>, bool),
) -> Row {
  let id = format!("{}-{}", form.ids.rows, spec.id);
  let label = ctx.t(spec.label);
  let options = options
    .into_iter()
    .map(|(option, words)| SelectOption::new(option, words))
    .collect();
  let mut select =
    SelectField::new(id.clone(), label.clone(), value.clone(), options).placeholder(ctx.t(spec.placeholder));
  if missing {
    select = select.invalid();
  }
  if form.locked || fixed {
    select = select.locked();
  }
  let sub = if missing {
    Some(Sub {
      words: ctx.t("team_new.errors.required"),
      color: PaletteColor::Danger,
    })
  } else {
    quiet.map(Sub::quiet)
  };
  rows::row(&format!("{id}-row"), &label, sub, select.render(ctx))
}
