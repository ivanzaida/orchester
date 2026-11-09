//! *Profile* of a local profile (`Profile` `Gk3js` in L1): the CLI it runs with, the connection it runs on, its model
//! and reasoning effort, the MCP servers it chooses and the secrets those take. The CLI, the connection, the model, the
//! effort and the servers are selects in place: each choice changes only what the panel holds until *Save*
//! ([`super::edit`]); the secrets are the servers' and only shown.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ConnectionId;
use orchester_workspace::agents::{local_edit::Edit, local_hub::LocalProfile};

use super::{
  pick::{self, Pick},
  servers,
};
use crate::{
  navigation::link::Open,
  screens::{
    hub::agent_row::tag_chip,
    team_hub::{
      profile::from_team::effort_words,
      rows::{self, Sub},
      words::cli_key,
    },
  },
  theme::{HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::SelectField,
    icon::{Icon, icon},
  },
};

/// What changing a field does.
pub(super) type OnEdit = Arc<dyn Fn(Edit) + Send + Sync>;

/// What the section needs of the panel.
pub(super) struct Rows<'a> {
  pub profile: &'a LocalProfile,
  /// The connection the person chose in the select, by id; empty while none is.
  pub chosen: &'a Signal<String>,
  pub on_edit: OnEdit,
  /// How many choices and saves ended: a select remounts on what the panel holds after each.
  pub settled: u64,
  /// A choice or a save is under way, or the hub is read again: nothing is changed now.
  pub busy: bool,
  /// Where *Add* next to the connection goes: the accounts, where a connection of another account is made.
  pub add_connection: Open,
}

pub(super) fn section(ctx: &mut Ctx, rows: &Rows<'_>) -> Column {
  let profile = rows.profile;
  let mut list = Vec::new();
  if profile.cli.is_some() || !profile.options.clis.is_empty() {
    list.push(cli(ctx, rows));
  }
  list.push(connection(ctx, rows));
  list.push(model(ctx, rows));
  list.push(effort(ctx, rows));
  list.push(mcp_servers(ctx, rows));
  if !profile.secrets.is_empty() {
    list.push(rows::row(
      "local-hub-profile-secrets",
      &ctx.t("team_hub.profile.secrets"),
      Some(Sub::quiet(ctx.t("team_hub.profile.secretsSub"))),
      rows::tags(
        "local-hub-profile-secrets-value",
        profile.secrets.iter().map(ToString::to_string),
      ),
    ));
  }
  rows::section("local-hub-profile", &ctx.t("local_hub.profile.title"), Vec::new(), list)
}

/// *Agent CLI*: the CLIs the project has a connection for; another one runs the profile on its first connection.
fn cli(ctx: &mut Ctx, rows: &Rows<'_>) -> Row {
  let profile = rows.profile;
  let clis = profile.options.clis.clone();
  let options = clis
    .iter()
    .enumerate()
    .map(|(index, cli)| (index.to_string(), ctx.t(cli_key(*cli)).to_string()))
    .collect();
  let value = profile
    .cli
    .and_then(|cli| clis.iter().position(|found| *found == cli))
    .map_or_else(String::new, |index| index.to_string());
  let edit = rows.on_edit.clone();
  // An option's value is its index in `clis`, so a choice always parses and names one of them.
  let on_pick: Pick = Arc::new(move |picked| {
    if let Some(cli) = picked.parse::<usize>().ok().and_then(|index| clis.get(index)) {
      edit(Edit::Cli(*cli));
    }
  });
  let label = ctx.t("team_hub.profile.agentCli");
  let select = pick::pick(
    ctx,
    pick::Props {
      id: "local-hub-profile-cli-pick".to_owned(),
      name: label.to_string(),
      value,
      options,
      placeholder: Some(ctx.t("local_hub.profile.chooseCli").to_string()),
      mono: false,
      locked: rows.busy,
      on_pick,
    },
    rows.settled,
  );
  rows::row("local-hub-profile-cli", &label, None, select)
}

/// *Model*: the models of its connection; none to choose from while the connection is gone.
fn model(ctx: &mut Ctx, rows: &Rows<'_>) -> Row {
  let profile = rows.profile;
  let listed = &profile.options.models;
  let options: Vec<(String, String)> = if listed.is_empty() {
    vec![(profile.model.clone(), profile.model.clone())]
  } else {
    listed.iter().map(|model| (model.clone(), model.clone())).collect()
  };
  let edit = rows.on_edit.clone();
  let on_pick: Pick = Arc::new(move |picked| edit(Edit::Model(picked)));
  let label = ctx.t("team_hub.profile.model");
  let select = pick::pick(
    ctx,
    pick::Props {
      id: "local-hub-profile-model-pick".to_owned(),
      name: label.to_string(),
      value: profile.model.clone(),
      options,
      placeholder: None,
      mono: true,
      locked: rows.busy || listed.is_empty(),
      on_pick,
    },
    rows.settled,
  );
  rows::row("local-hub-profile-model", &label, None, select)
}

/// *Reasoning effort*: the efforts its connection lists for its model, or the CLI's default.
fn effort(ctx: &mut Ctx, rows: &Rows<'_>) -> Row {
  let profile = rows.profile;
  let default = ctx.t("local_hub.profile.effortDefault").to_string();
  let mut options = vec![(String::new(), default)];
  options.extend(
    profile
      .options
      .efforts
      .iter()
      .map(|effort| (effort.clone(), effort_words(ctx, effort))),
  );
  let edit = rows.on_edit.clone();
  let on_pick: Pick = Arc::new(move |picked| edit(Edit::Effort(Some(picked).filter(|effort| !effort.is_empty()))));
  let label = ctx.t("team_hub.profile.effort");
  let select = pick::pick(
    ctx,
    pick::Props {
      id: "local-hub-profile-effort-pick".to_owned(),
      name: label.to_string(),
      value: profile.effort.clone().unwrap_or_default(),
      options,
      placeholder: None,
      mono: false,
      locked: rows.busy || profile.options.efforts.is_empty(),
      on_pick,
    },
    rows.settled,
  );
  rows::row("local-hub-profile-effort", &label, None, select)
}

/// *MCP servers*: the ones it chooses in tags, and *Add*, a menu of the project's servers.
fn mcp_servers(ctx: &mut Ctx, rows: &Rows<'_>) -> Row {
  let profile = rows.profile;
  let id = "local-hub-profile-mcp-value";
  let chosen = profile.mcp_servers.iter().map(|(server, name)| servers::Item {
    id: *server,
    name: name.clone(),
    chosen: true,
  });
  let others = profile.options.servers.iter().map(|(server, name)| servers::Item {
    id: *server,
    name: name.clone(),
    chosen: false,
  });
  let items: Vec<servers::Item> = chosen.chain(others).collect();
  let edit = rows.on_edit.clone();
  let on_toggle: servers::Toggle = Arc::new(move |server: ConnectionId, choose| {
    edit(if choose {
      Edit::AddServer(server)
    } else {
      Edit::RemoveServer(server)
    });
  });
  let tags: Vec<Element> = if profile.mcp_servers.is_empty() {
    vec![
      Text::new(&ctx.t("local_hub.profile.none"))
        .id(format!("{id}-none"))
        .variant(HubText::Value.role())
        .color(PaletteColor::TextMuted)
        .into(),
    ]
  } else {
    profile
      .mcp_servers
      .iter()
      .enumerate()
      .map(|(index, (_, name))| tag_chip(&format!("{id}-{index}"), name).into())
      .collect()
  };
  let add = (!items.is_empty()).then(|| {
    servers::add(
      ctx,
      servers::Props {
        items,
        label: ctx.t("local_hub.profile.addServer"),
        locked: rows.busy,
        on_toggle,
      },
    )
  });
  let value = Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .wrap()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(tags)
    .with_children(add);
  rows::row(
    "local-hub-profile-mcp",
    &ctx.t("team_hub.profile.mcpServers"),
    None,
    value,
  )
}

/// *Connection*: the one it runs on, chosen from the connections of its CLI when there are several or its own is gone.
fn connection(ctx: &Ctx, rows: &Rows<'_>) -> Row {
  let profile = rows.profile;
  let id = "local-hub-profile-connection";
  let label = ctx.t("local_hub.profile.connection");
  let Some(cli) = profile.cli else {
    return rows::row(
      id,
      &label,
      None,
      rows::words("local-hub-profile-connection-value", &ctx.t("local_hub.ready.ready")),
    );
  };
  let cli = ctx.t(cli_key(cli));
  let many = profile.connections.len();
  let (sub, value): (Option<Sub>, Element) = match (profile.connections.as_slice(), profile.current) {
    ([], _) => (
      None,
      Row::new()
        .id("local-hub-profile-connection-value")
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleX, IconSize::Base, PaletteColor::Danger))
        .child(
          Text::new(&ctx.t_args("team_hub.profile.noConnection", [("cli", &*cli)]))
            .variant(HubText::Value.role())
            .color(PaletteColor::Danger),
        )
        .into(),
    ),
    // Always a select, one connection or several, with *Add* beside it for another one.
    _ => (
      if many > 1 || profile.current.is_none() {
        Some(Sub {
          words: ctx.t_args("local_hub.profile.ofItsKind", [("count", many.to_string())]),
          color: if profile.current.is_none() {
            PaletteColor::Warning
          } else {
            PaletteColor::TextMuted
          },
        })
      } else {
        None
      },
      Row::new()
        .id("local-hub-profile-connection-value")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(select(
          ctx,
          profile,
          &cli,
          (rows.chosen, rows.busy),
        )))
        .child(
          ActionButton::new(
            "local-hub-profile-add-connection",
            ButtonKind::Ghost,
            ctx.t("local_hub.profile.addConnection"),
          )
          .icon(Icon::Plus)
          .small()
          .opens(rows.add_connection.clone()),
        )
        .into(),
    ),
  };
  rows::row(id, &label, sub, value)
}

/// The select of the connections of the CLI: the one chosen, else *Choose a Claude Code connection*.
fn select(ctx: &Ctx, profile: &LocalProfile, cli: &str, (chosen, locked): (&Signal<String>, bool)) -> Element {
  let program = profile
    .cli
    .and_then(orchester_core::connection::AgentProvider::program_name);
  let options = profile
    .connections
    .iter()
    .map(|choice| {
      let option = SelectOption::new(
        choice.id.to_string(),
        ctx.t_args(
          "team_hub.profile.connectionName",
          [("cli", cli), ("name", choice.name.as_str())],
        ),
      );
      match (&choice.version, program) {
        (Some(version), Some(program)) => option.detail(ctx.t_args(
          "team_hub.profile.cliVersion",
          [("program", program), ("version", version)],
        )),
        _ => option,
      }
    })
    .collect();
  let select = SelectField::new(
    "local-hub-profile-pick",
    ctx.t("local_hub.profile.connection"),
    chosen.clone(),
    options,
  )
  .placeholder(ctx.t_args("team_hub.profile.choose", [("cli", cli)]));
  if locked { select.locked() } else { select }.render(ctx)
}
