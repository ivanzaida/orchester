//! The form of a new agent profile (`Agents · New agent profile` A7 `Cijpv`, in a local project L3 `JEkCL`): its name,
//! what it is for, the CLI kind and model (and effort) it runs with. In a team's project which connection runs it is
//! chosen on each computer afterwards, so the CLI is a kind, one choice for each CLI this computer has a connection
//! of; the model and effort are those that connection lists. In a local project everything is the person's own, so
//! the profile names its connection (chosen for it when the CLI has one, else from those of its kind) and the MCP
//! servers it chooses. The instructions, tools and skills a profile picks from the lists are not in this form yet: a
//! profile of this computer cannot hold them, and the form says so.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{
    AgentConnection,
    invocation::Platform,
    mcp::{McpSelectionError, McpServerSelection},
  },
  id::ConnectionId,
  permission::AgentPermissions,
  resource::ModelId,
};
use orchester_workspace::agents::form::{
  ModelOption, ModelSelection, ProfileForm, access_for_connection, effort_options, model_options,
};

use super::{
  fields::{self, Body, Error, Look, Shown, Spec, name_error},
  work::Make,
};
use crate::{
  screens::{
    hub::agent_row::tag_chip,
    local_hub::profile::servers,
    team_hub::{rows, words::cli_key},
  },
  theme::HubText,
};

const NAME: Spec = Spec::new("profile-name", "team_new.profile.name")
  .sub("team_new.profile.nameSub")
  .required();
const RESPONSIBILITY: Spec = Spec::new("profile-responsibility", "team_new.profile.responsibility");
const CLI: Spec = Spec::new("profile-cli", "team_new.profile.cli")
  .sub("team_new.profile.cliSub")
  .required();
const CLI_LOCAL: Spec = Spec::new("profile-cli", "team_new.profile.cli").required();
const MODEL: Spec = Spec::new("profile-model", "team_new.profile.model").required();
const EFFORT: Spec = Spec::new("profile-effort", "team_new.profile.effort");
const CONNECTION: Spec =
  Spec::new("profile-connection", "local_new.profile.connection").sub("local_new.profile.connectionSub");
const SERVERS: Spec = Spec::new("profile-servers", "local_new.profile.servers");
const LISTS: Spec = Spec::new("profile-lists", "team_new.profile.lists").sub("team_new.profile.listsSub");
const LISTS_LOCAL: Spec = Spec::new("profile-lists", "local_new.profile.lists").sub("local_new.profile.listsSub");

/// The MCP servers a local profile chooses, in the order they were chosen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Chosen(Vec<ConnectionId>);
impl DevtoolsInspectable for Chosen {}

pub(super) struct Fields {
  name: Signal<String>,
  responsibility: Signal<String>,
  /// The connection that stands for the CLI chosen, by id: empty while none is.
  cli: Signal<String>,
  /// The connection a local profile runs on, by id, when its CLI has several: empty until one is chosen.
  connection: Signal<String>,
  /// The MCP servers a local profile chooses.
  servers: Signal<Chosen>,
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
      connection: ctx.signal(String::new()),
      servers: ctx.signal(Chosen::default()),
      model: ctx.signal(String::new()),
      effort: ctx.signal(String::new()),
    }
  }

  pub(super) fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Body {
    let name = self.name.get();
    let local = shown.mode.is_local();
    let all: Vec<&AgentConnection> = shown.agents.map_or_else(Vec::new, |agents| {
      agents
        .connections
        .iter()
        .filter(|connection| connection.provider().runs_agents())
        .collect()
    });
    let connections = one_of_each_cli(&all);
    let standing_for_cli = connections
      .iter()
      .find(|c| c.id().to_string() == self.cli.get())
      .copied();
    // Which one runs it: a team's profile is on the connection that stands for its CLI, a local one on its own.
    let own = if local {
      self.own_connection(&all, standing_for_cli)
    } else {
      standing_for_cli
    };
    let models = own.map_or_else(Vec::new, |c| model_options(c, None));
    let model = models
      .iter()
      .find(|option| option.id.as_str() == self.model.get())
      .map(|o| o.id.clone());
    let efforts = match (own, &model) {
      (Some(connection), Some(model)) => effort_options(connection, model),
      _ => Vec::new(),
    };
    let name_says = name_error(
      ctx,
      shown,
      &name,
      shown
        .mode
        .words("team_new.profile.nameTaken", "local_new.nameTaken.profile"),
    );
    let cli_says = (shown.tried && standing_for_cli.is_none()).then_some(Error::Required);
    let model_says = (shown.tried && model.is_none()).then_some(Error::Required);
    let mut rows = self.head_rows(ctx, shown, name_says.as_ref());
    let cli_options: Vec<(String, String)> = connections
      .iter()
      .map(|c| (c.id().to_string(), ctx.t(cli_key(c.provider())).to_string()))
      .collect();
    rows.push(self.cli_row(ctx, shown, cli_options, cli_says.as_ref()));
    if local {
      rows.push(self.connection_row(ctx, shown, (&all, standing_for_cli), own));
    }
    rows.push(self.model_row(ctx, shown, &models, model_says.as_ref()));
    rows.push(self.effort_row(ctx, &efforts, shown.locked));
    let chosen = if local {
      McpServerSelection::new(self.servers.get().0)
    } else {
      Ok(McpServerSelection::none())
    };
    if local {
      rows.push(self.servers_row(ctx, shown, chosen.as_ref().err()));
    }
    rows.push(fields::row(
      ctx,
      if local { &LISTS_LOCAL } else { &LISTS },
      None,
      rows::words(
        "team-new-profile-lists-value",
        &ctx.t(
          shown
            .mode
            .words("team_new.profile.listsValue", "local_new.profile.listsValue"),
        ),
      ),
    ));
    Body {
      rows,
      computer: Some("team_new.computer.profile"),
      make: own
        .zip(chosen.ok())
        .and_then(|(c, servers)| self.made(c, model, servers, local)),
      off: fields::off(shown, &name),
      overlay: None,
    }
  }

  /// The name and the responsibility, which both modes ask.
  fn head_rows(&self, ctx: &mut Ctx, shown: &Shown<'_>, name_says: Option<&Error>) -> Vec<Row> {
    vec![
      fields::name_row(
        ctx,
        shown,
        &NAME,
        name_says,
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
    ]
  }

  /// The connection a local profile runs on: the only one of its CLI, else the one chosen among those.
  fn own_connection<'a>(
    &self,
    all: &[&'a AgentConnection],
    standing_for_cli: Option<&AgentConnection>,
  ) -> Option<&'a AgentConnection> {
    let provider = standing_for_cli?.provider();
    let of_kind: Vec<&AgentConnection> = all
      .iter()
      .filter(|connection| connection.provider() == provider)
      .copied()
      .collect();
    match of_kind.as_slice() {
      [only] => Some(*only),
      several => several
        .iter()
        .find(|connection| connection.id().to_string() == self.connection.get())
        .copied(),
    }
  }

  fn cli_row(&self, ctx: &Ctx, shown: &Shown<'_>, options: Vec<(String, String)>, says: Option<&Error>) -> Row {
    // A team's CLI is a kind, never a connection, which its line says; a local profile names its connection apart.
    let spec = if shown.mode.is_local() { &CLI_LOCAL } else { &CLI };
    fields::row(
      ctx,
      spec,
      says,
      fields::select(
        ctx,
        spec,
        &self.cli,
        options,
        ("team_new.profile.cliPlaceholder", says.is_some(), shown.locked),
      ),
    )
  }

  /// *Connection* of a local profile: chosen for it when its CLI has one, else asked from those of its kind.
  fn connection_row(
    &self,
    ctx: &Ctx,
    shown: &Shown<'_>,
    (all, standing_for_cli): (&[&AgentConnection], Option<&AgentConnection>),
    own: Option<&AgentConnection>,
  ) -> Row {
    let of_kind: Vec<&AgentConnection> = standing_for_cli.map_or_else(Vec::new, |standing| {
      all
        .iter()
        .filter(|connection| connection.provider() == standing.provider())
        .copied()
        .collect()
    });
    let asked = of_kind.len() > 1;
    let says = (shown.tried && asked && own.is_none()).then_some(Error::Required);
    let options = of_kind
      .iter()
      .map(|c| (c.id().to_string(), c.name().as_str().to_owned()))
      .collect();
    let control = fields::select(
      ctx,
      &CONNECTION,
      &self.connection,
      options,
      (
        match (standing_for_cli.is_some(), asked) {
          (true, true) => "local_new.profile.connectionAsk",
          (true, false) => "local_new.profile.connectionOnly",
          (false, _) => "local_new.profile.connectionAuto",
        },
        says.is_some(),
        shown.locked || !asked,
      ),
    );
    fields::row(ctx, &CONNECTION, says.as_ref(), control)
  }

  fn model_row(&self, ctx: &Ctx, shown: &Shown<'_>, models: &[ModelOption], says: Option<&Error>) -> Row {
    let options = models
      .iter()
      .map(|option| (option.id.as_str().to_owned(), option.id.as_str().to_owned()))
      .collect();
    fields::row(
      ctx,
      &MODEL,
      says,
      fields::select(
        ctx,
        &MODEL,
        &self.model,
        options,
        ("team_new.profile.modelPlaceholder", says.is_some(), shown.locked),
      ),
    )
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

  /// *MCP servers* of a local profile: the ones it chooses in tags, and a menu of the project's servers.
  fn servers_row(&self, ctx: &mut Ctx, shown: &Shown<'_>, refused: Option<&McpSelectionError>) -> Row {
    let available = shown
      .agents
      .map_or_else(Vec::new, |agents| agents.mcp.iter().cloned().collect());
    let chosen = self.servers.get().0;
    let id = "team-new-profile-servers-value";
    let tags: Vec<Element> = if chosen.is_empty() {
      vec![
        Text::new(&ctx.t("local_new.profile.serversNone"))
          .id(format!("{id}-none"))
          .variant(HubText::Value.role())
          .color(PaletteColor::TextMuted)
          .into(),
      ]
    } else {
      chosen
        .iter()
        .enumerate()
        .filter_map(|(index, server)| {
          let found = available.iter().find(|found| found.id() == *server)?;
          Some(tag_chip(&format!("{id}-{index}"), found.name().as_str()).into())
        })
        .collect()
    };
    let items: Vec<servers::Item> = available
      .iter()
      .map(|server| servers::Item {
        id: server.id(),
        name: server.name().as_str().to_owned(),
        chosen: chosen.contains(&server.id()),
      })
      .collect();
    let signal = self.servers.clone();
    let on_toggle: servers::Toggle = std::sync::Arc::new(move |server, choose| {
      signal.update(|chosen| {
        chosen.0.retain(|found| *found != server);
        if choose {
          chosen.0.push(server);
        }
      });
    });
    let add = (!items.is_empty()).then(|| {
      servers::add(
        ctx,
        servers::Props {
          items,
          label: ctx.t("local_hub.profile.addServer"),
          locked: shown.locked,
          on_toggle,
        },
      )
    });
    let value = Row::new()
      .id(id)
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(lurq::app::theme::SpacingSize::Sm)
      .align_items(Alignment::Center)
      .with_children(tags)
      .with_children(add);
    let error = refused.map(|refused| {
      Error::Words(match refused {
        McpSelectionError::TooMany => ctx.t_args(
          "local_new.profile.serversTooMany",
          [("max", McpServerSelection::MAX.to_string())],
        ),
        McpSelectionError::Duplicate(_) => ctx.t("local_new.profile.serversDuplicate"),
      })
    });
    fields::row(ctx, &SERVERS, error.as_ref(), value)
  }

  /// The profile the fields make on `connection`: the most restrictive permissions, and the MCP servers a local
  /// profile chose (`servers`; a team's has none).
  fn made(
    &self,
    connection: &AgentConnection,
    model: Option<ModelId>,
    servers: McpServerSelection,
    local: bool,
  ) -> Option<Make> {
    let selection = model.map_or(ModelSelection::None, ModelSelection::Listed);
    let effort = self.effort.get();
    let platform = Platform::current();
    let permissions = AgentPermissions::most_restrictive(connection.provider(), platform);
    // A native connection takes neither CLI approval switches nor MCP servers.
    let (permissions, servers) = if local {
      access_for_connection(&permissions, &servers, connection.provider(), platform)
    } else {
      (permissions, servers)
    };
    ProfileForm {
      role: &self.name.get(),
      responsibilities: &self.responsibility.get(),
      instructions: "",
      connection: Some(connection),
      model: &selection,
      custom_model: "",
      effort: Some(effort.as_str()).filter(|effort| !effort.is_empty()),
      permissions: &permissions,
      mcp_servers: &servers,
    }
    .profile()
    // Why it is not valid is said at each field's row, which reads the same form.
    .ok()
    .map(|profile| Make::Profile(Box::new(profile)))
  }
}

/// The first connection of each CLI kind, which stands for the kind in the CLI select.
fn one_of_each_cli<'a>(all: &[&'a AgentConnection]) -> Vec<&'a AgentConnection> {
  let mut chosen: Vec<&AgentConnection> = Vec::new();
  for connection in all {
    if !chosen.iter().any(|found| found.provider() == connection.provider()) {
      chosen.push(connection);
    }
  }
  chosen
}
