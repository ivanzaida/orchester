//! *On this computer* (`On this computer` `qtmpw` in `Agents · Profile` `NDZXC`, `WPvZx`, `QOBF2`): what is only this
//! computer's and never sent to the team. Today that is the connection the profile runs on: the only one of its CLI
//! kind without asking (C1), a pick among several, asked once and kept (C2), or none, which says what to add and
//! substitutes nothing (C3).

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
use orchester_workspace::agents::team_profile::{ConnectionChoice, TeamProfileSheet};

use super::rows::{self, Sub};
use crate::{
  screens::team_hub::words::cli_key,
  theme::{HubText, IconSize},
  ui::{
    agent_mark::AgentMark,
    chip::{BadgeTone, badge},
    field::SelectField,
    icon::{Icon, icon},
  },
};

pub(super) fn section(ctx: &Ctx, sheet: &TeamProfileSheet, chosen: &Signal<String>) -> Column {
  let cli = ctx.t(cli_key(sheet.cli));
  let (sub, value) = runs_on(ctx, sheet, &cli, chosen);
  let note = Text::new(&ctx.t("team_hub.profile.onComputerNote"))
    .id("team-hub-on-computer-note")
    .variant(HubText::Hint.role())
    .nowrap();
  rows::section(
    "team-hub-on-computer",
    &ctx.t("team_hub.profile.onComputer"),
    vec![
      badge("team-hub-yours", BadgeTone::Neutral, &ctx.t("team_hub.profile.yours")).into(),
      note.into(),
    ],
    vec![rows::row(
      "team-hub-runs-on",
      &ctx.t("team_hub.profile.runsOn"),
      Some(sub),
      value,
    )],
  )
}

/// What stands under *Runs on* and what the row's value is.
fn runs_on(ctx: &Ctx, sheet: &TeamProfileSheet, cli: &str, chosen: &Signal<String>) -> (Sub, Element) {
  match sheet.connections.as_slice() {
    [] => (
      Sub::quiet(ctx.t("team_hub.profile.runsOnNone")),
      Row::new()
        .id("team-hub-runs-on-value")
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleX, IconSize::Base, PaletteColor::Danger))
        .child(
          Text::new(&ctx.t_args("team_hub.profile.noConnection", [("cli", cli)]))
            .id("team-hub-runs-on-none")
            .variant(HubText::Value.role())
            .color(PaletteColor::Danger),
        )
        .into(),
    ),
    [only] => (
      Sub::quiet(ctx.t("team_hub.profile.runsOnOnly")),
      only_one(ctx, sheet, only, cli),
    ),
    many => {
      let sub = if sheet.picked.is_some() {
        Sub::quiet(ctx.t("team_hub.profile.runsOnPicked"))
      } else {
        Sub {
          words: ctx.t_args(
            "team_hub.profile.runsOnMany",
            [("count", many.len().to_string()), ("cli", cli.to_owned())],
          ),
          color: PaletteColor::Warning,
        }
      };
      (sub, select(ctx, sheet, cli, chosen))
    }
  }
}

/// *Claude Code · Claude 2 · the only Claude Code connection here*.
fn only_one(ctx: &Ctx, sheet: &TeamProfileSheet, only: &ConnectionChoice, cli: &str) -> Element {
  Row::new()
    .id("team-hub-runs-on-value")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(AgentMark::of(sheet.cli).render(ctx, IconSize::Lg))
    .child(
      Text::new(&ctx.t_args(
        "team_hub.profile.connectionName",
        [("cli", cli), ("name", only.name.as_str())],
      ))
      .id("team-hub-runs-on-name")
      .variant(HubText::Value.role()),
    )
    .child(
      Text::new(&ctx.t_args("team_hub.profile.onlyOne", [("cli", cli)]))
        .id("team-hub-runs-on-only")
        .variant(HubText::Hint.role()),
    )
    .into()
}

/// The pick among several: *Choose a Claude Code connection*, or the one picked.
fn select(ctx: &Ctx, sheet: &TeamProfileSheet, cli: &str, chosen: &Signal<String>) -> Element {
  let options = sheet
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
      match (&choice.version, sheet.cli.program_name()) {
        (Some(version), Some(program)) => option.detail(ctx.t_args(
          "team_hub.profile.cliVersion",
          [("program", program), ("version", version)],
        )),
        _ => option,
      }
    })
    .collect();
  SelectField::new(
    "team-hub-pick",
    ctx.t("team_hub.profile.runsOn"),
    chosen.clone(),
    options,
  )
  .placeholder(ctx.t_args("team_hub.profile.choose", [("cli", cli)]))
  .render(ctx)
}
