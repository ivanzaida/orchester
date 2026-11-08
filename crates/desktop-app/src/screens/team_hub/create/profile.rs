//! The form of a new agent profile (`Agents · New agent profile` A7 `Cijpv`): what the team holds of it, its name, what
//! it is for, the CLI kind and model (and effort) it runs with. Which connection runs it is chosen on each computer
//! afterwards, so the CLI is a kind, one choice for each CLI this computer has a connection of; the model and effort
//! are those that connection lists. The instructions, MCP servers, tools and skills a profile picks from the team's
//! lists are not in this form yet: a profile of this computer cannot hold them, and the form says so.

use lurq::{app::ctx::Ctx, components::Row, core::Signal};
use orchester_core::{
  connection::{AgentConnection, invocation::Platform, mcp::McpServerSelection},
  permission::AgentPermissions,
  resource::ModelId,
};
use orchester_workspace::agents::form::{ModelOption, ModelSelection, ProfileForm, effort_options, model_options};

use super::{
  fields::{self, Body, Error, Look, Shown, Spec, name_error},
  work::Make,
};
use crate::screens::team_hub::{rows, words::cli_key};

const NAME: Spec = Spec::new("profile-name", "team_new.profile.name")
  .sub("team_new.profile.nameSub")
  .required();
const RESPONSIBILITY: Spec = Spec::new("profile-responsibility", "team_new.profile.responsibility");
const CLI: Spec = Spec::new("profile-cli", "team_new.profile.cli")
  .sub("team_new.profile.cliSub")
  .required();
const MODEL: Spec = Spec::new("profile-model", "team_new.profile.model").required();
const EFFORT: Spec = Spec::new("profile-effort", "team_new.profile.effort");
const LISTS: Spec = Spec::new("profile-lists", "team_new.profile.lists").sub("team_new.profile.listsSub");

pub(super) struct Fields {
  name: Signal<String>,
  responsibility: Signal<String>,
  /// The connection that stands for the CLI chosen, by id: empty while none is.
  cli: Signal<String>,
  model: Signal<String>,
  /// The effort chosen; empty is the CLI's default.
  effort: Signal<String>,
}

impl Fields {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      name: ctx.signal(String::new()),
      responsibility: ctx.signal(String::new()),
      cli: ctx.signal(String::new()),
      model: ctx.signal(String::new()),
      effort: ctx.signal(String::new()),
    }
  }

  pub(super) fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Body {
    let name = self.name.get();
    let connections: Vec<&AgentConnection> = shown.agents.map_or_else(Vec::new, |agents| {
      let mut chosen: Vec<&AgentConnection> = Vec::new();
      for connection in agents.connections.iter().filter(|c| c.provider().runs_agents()) {
        if !chosen.iter().any(|found| found.provider() == connection.provider()) {
          chosen.push(connection);
        }
      }
      chosen
    });
    let connection = connections
      .iter()
      .find(|c| c.id().to_string() == self.cli.get())
      .copied();
    let models = connection.map_or_else(Vec::new, |c| model_options(c, None));
    let model = models
      .iter()
      .find(|option| option.id.as_str() == self.model.get())
      .map(|o| o.id.clone());
    let efforts = match (connection, &model) {
      (Some(connection), Some(model)) => effort_options(connection, model),
      _ => Vec::new(),
    };
    let name_says = name_error(ctx, shown, &name, "team_new.profile.nameTaken");
    let cli_says = (shown.tried && connection.is_none()).then_some(Error::Required);
    let model_says = (shown.tried && model.is_none()).then_some(Error::Required);
    let mut rows = vec![
      fields::name_row(
        ctx,
        shown,
        &NAME,
        name_says.as_ref(),
        (&self.name, "team_new.profile.namePlaceholder"),
        Look::Plain,
      ),
      fields::text_row(
        ctx,
        &RESPONSIBILITY,
        None,
        (&self.responsibility, "team_new.profile.responsibilityPlaceholder"),
        (Look::Plain, shown.locked),
      ),
    ];
    rows.extend(Self::choice_rows(
      ctx,
      shown,
      (&connections, &models),
      (cli_says, model_says),
      (&self.cli, &self.model),
    ));
    rows.push(self.effort_row(ctx, &efforts, shown.locked));
    rows.push(fields::row(
      ctx,
      &LISTS,
      None,
      rows::words("team-new-profile-lists-value", &ctx.t("team_new.profile.listsValue")),
    ));
    Body {
      rows,
      computer: "team_new.computer.profile",
      make: connection.and_then(|c| self.made(c, model)),
      off: fields::taken(shown, &name).map(|_| fields::FIX_NAME),
    }
  }

  /// The CLI and the model: a select each, the model's options those the CLI's connection lists.
  fn choice_rows(
    ctx: &Ctx,
    shown: &Shown<'_>,
    (connections, models): (&[&AgentConnection], &[ModelOption]),
    (cli_says, model_says): (Option<Error>, Option<Error>),
    (cli, model): (&Signal<String>, &Signal<String>),
  ) -> Vec<Row> {
    let cli_options = connections
      .iter()
      .map(|c| (c.id().to_string(), ctx.t(cli_key(c.provider())).to_string()))
      .collect();
    let model_options = models
      .iter()
      .map(|option| (option.id.as_str().to_owned(), option.id.as_str().to_owned()))
      .collect();
    vec![
      fields::row(
        ctx,
        &CLI,
        cli_says.as_ref(),
        fields::select(
          ctx,
          &CLI,
          cli,
          cli_options,
          ("team_new.profile.cliPlaceholder", cli_says.is_some(), shown.locked),
        ),
      ),
      fields::row(
        ctx,
        &MODEL,
        model_says.as_ref(),
        fields::select(
          ctx,
          &MODEL,
          model,
          model_options,
          ("team_new.profile.modelPlaceholder", model_says.is_some(), shown.locked),
        ),
      ),
    ]
  }

  fn effort_row(&self, ctx: &Ctx, efforts: &[String], locked: bool) -> Row {
    let mut options = vec![(String::new(), ctx.t("team_new.profile.effortDefault").to_string())];
    options.extend(efforts.iter().map(|effort| (effort.clone(), effort.clone())));
    fields::row(
      ctx,
      &EFFORT,
      None,
      fields::select(
        ctx,
        &EFFORT,
        &self.effort,
        options,
        ("team_new.profile.effortDefault", false, locked),
      ),
    )
  }

  /// The profile the fields make on the connection that stands for its CLI: the most restrictive permissions and no
  /// MCP server, as a profile starts.
  fn made(&self, connection: &AgentConnection, model: Option<ModelId>) -> Option<Make> {
    let selection = model.map_or(ModelSelection::None, ModelSelection::Listed);
    let effort = self.effort.get();
    let permissions = AgentPermissions::most_restrictive(connection.provider(), Platform::current());
    ProfileForm {
      role: &self.name.get(),
      responsibilities: &self.responsibility.get(),
      instructions: "",
      connection: Some(connection),
      model: &selection,
      custom_model: "",
      effort: Some(effort.as_str()).filter(|effort| !effort.is_empty()),
      permissions: &permissions,
      mcp_servers: &McpServerSelection::none(),
    }
    .profile()
    // Why it is not valid is said at each field's row, which reads the same form.
    .ok()
    .map(|profile| Make::Profile(Box::new(profile)))
  }
}
