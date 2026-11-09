//! *Profile* of a local profile (`Profile` `Gk3js` in L1): the CLI it runs with, the connection it runs on, its model
//! and reasoning effort, the MCP servers it chooses and the secrets those take. Only the connection is chosen in the
//! panel, among the connections of the profile's CLI; the rest is what its editor sets (*Edit*), shown as the team's
//! panel shows it.

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
use orchester_workspace::agents::local_hub::LocalProfile;

use crate::{
  screens::team_hub::{
    profile::from_team::effort_words,
    rows::{self, Sub},
    words::cli_key,
  },
  theme::{HubText, IconSize},
  ui::{
    agent_mark::AgentMark,
    field::SelectField,
    icon::{Icon, icon},
  },
};

pub(super) fn section(ctx: &Ctx, profile: &LocalProfile, chosen: &Signal<String>) -> Column {
  let mut rows = Vec::new();
  if let Some(cli) = profile.cli {
    rows.push(rows::row(
      "local-hub-profile-cli",
      &ctx.t("team_hub.profile.agentCli"),
      None,
      rows::words("local-hub-profile-cli-value", &ctx.t(cli_key(cli))),
    ));
  }
  rows.push(connection(ctx, profile, chosen));
  rows.push(rows::row(
    "local-hub-profile-model",
    &ctx.t("team_hub.profile.model"),
    None,
    rows::mono("local-hub-profile-model-value", &profile.model),
  ));
  let effort = profile.effort.as_deref().map_or_else(
    || ctx.t("local_hub.profile.effortDefault").to_string(),
    |effort| effort_words(ctx, effort),
  );
  rows.push(rows::row(
    "local-hub-profile-effort",
    &ctx.t("team_hub.profile.effort"),
    None,
    rows::words("local-hub-profile-effort-value", &effort),
  ));
  rows.push(rows::row(
    "local-hub-profile-mcp",
    &ctx.t("team_hub.profile.mcpServers"),
    None,
    listed(
      ctx,
      "local-hub-profile-mcp-value",
      profile.mcp_servers.iter().map(|(_, name)| name.clone()),
    ),
  ));
  if !profile.secrets.is_empty() {
    rows.push(rows::row(
      "local-hub-profile-secrets",
      &ctx.t("team_hub.profile.secrets"),
      Some(Sub::quiet(ctx.t("team_hub.profile.secretsSub"))),
      rows::tags(
        "local-hub-profile-secrets-value",
        profile.secrets.iter().map(ToString::to_string),
      ),
    ));
  }
  rows::section("local-hub-profile", &ctx.t("local_hub.profile.title"), Vec::new(), rows)
}

/// The names in tags, or that there are none.
fn listed(ctx: &Ctx, id: &str, names: impl Iterator<Item = String>) -> Element {
  let names: Vec<String> = names.collect();
  if names.is_empty() {
    return rows::words(id, &ctx.t("local_hub.profile.none"))
      .color(PaletteColor::TextMuted)
      .into();
  }
  rows::tags(id, names).into()
}

/// *Connection*: the one it runs on, chosen from the connections of its CLI when there are several or its own is gone.
fn connection(ctx: &Ctx, profile: &LocalProfile, chosen: &Signal<String>) -> Row {
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
    ([only], Some(_)) => (None, only_one(ctx, profile, &only.name, &cli)),
    _ => (
      Some(Sub {
        words: ctx.t_args("local_hub.profile.ofItsKind", [("count", many.to_string())]),
        color: if profile.current.is_none() {
          PaletteColor::Warning
        } else {
          PaletteColor::TextMuted
        },
      }),
      select(ctx, profile, &cli, chosen),
    ),
  };
  rows::row(id, &label, sub, value)
}

/// *Claude Code · Claude 2*, after the CLI's mark.
fn only_one(ctx: &Ctx, profile: &LocalProfile, name: &str, cli: &str) -> Element {
  let mark = profile.cli.map(|cli| AgentMark::of(cli).render(ctx, IconSize::Lg));
  Row::new()
    .id("local-hub-profile-connection-value")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(mark)
    .child(
      Text::new(&ctx.t_args("team_hub.profile.connectionName", [("cli", cli), ("name", name)]))
        .variant(HubText::Value.role()),
    )
    .into()
}

/// The select of the connections of the CLI: the one chosen, else *Choose a Claude Code connection*.
fn select(ctx: &Ctx, profile: &LocalProfile, cli: &str, chosen: &Signal<String>) -> Element {
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
  SelectField::new(
    "local-hub-profile-pick",
    ctx.t("local_hub.profile.connection"),
    chosen.clone(),
    options,
  )
  .placeholder(ctx.t_args("team_hub.profile.choose", [("cli", cli)]))
  .render(ctx)
}
