//! *On this computer* (`On this computer` `qtmpw` in `Agents · Profile` `NDZXC`, `WPvZx`, `QOBF2`; `Agents on
//! accounts` C3 `OTVQt`): what is only this computer's and never sent to the team. Today that is the account the
//! profile runs on: this computer's accounts of its CLI, each once, then what runs on a key
//! ([`here`](crate::screens::hub::profile_form::here)). The only one runs it without asking; among several the person
//! picks once and it is kept; an account that lists other models than the team's is shown and cannot be picked; with
//! none, the row says what to add and substitutes nothing.

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
use orchester_workspace::agents::team_profile::TeamProfileSheet;

use crate::{
  screens::{
    hub::profile_form::{Here, here::Offered},
    team_hub::{
      rows::{self, Sub},
      words::cli_key,
    },
  },
  theme::{HubText, IconSize},
  ui::{
    agent_mark::AgentMark,
    field::SelectField,
    icon::{Icon, icon},
  },
};

/// The value the select holds for *Add an account…*, which opens the accounts and chooses nothing.
pub(super) const ADD: &str = "add";

pub(super) fn section(ctx: &Ctx, sheet: &TeamProfileSheet, (offered, chosen): (&[Offered], &Signal<String>)) -> Column {
  let cli = ctx.t(cli_key(sheet.cli));
  let (sub, value) = runs_on(ctx, sheet, &cli, (offered, chosen));
  rows::on_this_computer(
    ctx,
    vec![rows::row(
      "team-hub-runs-on",
      &ctx.t("team_hub.profile.runsOn"),
      Some(sub),
      value,
    )],
    None,
  )
}

/// What stands under *Runs on* and what the row's value is.
fn runs_on(
  ctx: &Ctx,
  sheet: &TeamProfileSheet,
  cli: &str,
  (offered, chosen): (&[Offered], &Signal<String>),
) -> (Sub, Element) {
  match offered {
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
    // The only one runs it without asking once the project has its one connection on it.
    [only] if matches!(only.here, Here::Connection(_)) && !only.other_models && sheet.runs_on().is_some() => (
      Sub::quiet(ctx.t("team_hub.profile.runsOnOnly")),
      only_one(ctx, sheet, only, cli),
    ),
    several => {
      let sub = if sheet.picked.is_some() {
        Sub::quiet(ctx.t("team_hub.profile.runsOnPicked"))
      } else {
        Sub {
          words: ctx.t("team_hub.runsHere.pick"),
          color: PaletteColor::Warning,
        }
      };
      (sub, select(ctx, cli, (several, chosen)))
    }
  }
}

/// *Claude Code · Default · the only Claude Code account here*.
fn only_one(ctx: &Ctx, sheet: &TeamProfileSheet, only: &Offered, cli: &str) -> Element {
  Row::new()
    .id("team-hub-runs-on-value")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(AgentMark::of(sheet.cli).render(ctx, IconSize::Lg))
    .child(
      Text::new(&only.label)
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

/// The pick among several: *Choose a Claude Code account*, or the one picked; each with its plan, one that lists
/// other models unavailable, and *Add an account…* last.
fn select(ctx: &Ctx, cli: &str, (offered, chosen): (&[Offered], &Signal<String>)) -> Element {
  let mut options: Vec<SelectOption<String>> = offered
    .iter()
    .map(|offered| {
      let option = SelectOption::new(offered.value.clone(), offered.label.to_string()).disabled(offered.other_models);
      match &offered.detail {
        Some(detail) => option.detail(detail.clone()),
        None => option,
      }
    })
    .collect();
  options.push(SelectOption::new(
    ADD.to_owned(),
    ctx.t("team_hub.runsHere.add").to_string(),
  ));
  SelectField::new(
    "team-hub-pick",
    ctx.t("team_hub.profile.runsOn"),
    chosen.clone(),
    options,
  )
  .placeholder(ctx.t_args("team_hub.runsHere.choose", [("cli", cli)]))
  .render(ctx)
}
