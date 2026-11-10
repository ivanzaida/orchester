//! What an archived local profile was, read-only until it is restored (`Agents · Archived resource · Restore` L14
//! `a6ZLLg`, as an archived MCP server shows its fields): its role and what it ran with and gave its agents, as values,
//! *None* where it had nothing.

use lurq::{
  app::ctx::Ctx,
  components::{Column, Text},
};
use orchester_workspace::agents::{local_given::Named, local_hub::LocalProfile};

use super::archived::read_row;
use crate::{
  screens::team_hub::{profile::from_team::effort_words, rows, words::cli_key},
  theme::HubText,
};

/// *Profile · read-only until it is restored* over what the profile was.
pub(super) fn read_only(ctx: &Ctx, profile: &LocalProfile) -> Column {
  let none = ctx.t("local_hub.profile.none").to_string();
  let or_none = |names: Vec<String>| {
    if names.is_empty() {
      none.clone()
    } else {
      names.join(", ")
    }
  };
  let cli = profile
    .cli
    .map_or_else(|| none.clone(), |cli| ctx.t(cli_key(cli)).to_string());
  let effort = profile.effort.as_deref().map_or_else(
    || ctx.t("local_hub.profile.effortDefault").to_string(),
    |effort| effort_words(ctx, effort),
  );
  let given = |list: &[Named]| or_none(list.iter().map(|(_, name)| name.clone()).collect());
  let rows = vec![
    read_row("role", &ctx.t("hub.role.name"), &profile.role.role, false),
    read_row("cli", &ctx.t("team_hub.profile.agentCli"), &cli, false),
    read_row(
      "connection",
      &ctx.t("local_hub.profile.connection"),
      profile.connection.as_deref().unwrap_or(&none),
      false,
    ),
    read_row("model", &ctx.t("team_hub.profile.model"), &profile.model, true),
    read_row("effort", &ctx.t("team_hub.profile.effort"), &effort, false),
    read_row(
      "mcp",
      &ctx.t("team_hub.profile.mcpServers"),
      &or_none(profile.mcp_servers.iter().map(|(_, name)| name.clone()).collect()),
      false,
    ),
    read_row(
      "instructions",
      &ctx.t("local_hub.profile.instructions"),
      &given(&profile.given.instructions),
      false,
    ),
    read_row(
      "skills",
      &ctx.t("local_hub.profile.skills"),
      &given(&profile.given.skills),
      false,
    ),
    read_row(
      "tools",
      &ctx.t("local_hub.profile.tools"),
      &given(&profile.given.tools),
      false,
    ),
  ];
  rows::section(
    "local-hub-archived-profile",
    &ctx.t("local_hub.profile.title"),
    vec![
      Text::new(&ctx.t("local_hub.archived.readOnly"))
        .id("local-hub-archived-read-only")
        .variant(HubText::PanelHint.role())
        .nowrap()
        .into(),
    ],
    rows,
  )
}
