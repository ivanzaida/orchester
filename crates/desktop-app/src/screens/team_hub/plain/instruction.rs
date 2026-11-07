//! What an instruction's panel shows besides its head (`From the team` and `Text` in `Agents · Instructions`
//! `c3jU1R`): where it is added, the roles or profiles that get it, when, who uses it, its format and its text.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Text},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::InjectionEvent;
use orchester_workspace::agents::team_text::{Reaches, TeamInstructionSheet};

use super::{added_row, from_team, used_by_row};
use crate::{
  screens::team_hub::rows,
  theme::{HubSpacing, HubText},
};

pub(super) fn sections(ctx: &Ctx, sheet: &TeamInstructionSheet) -> Vec<Element> {
  let mut rows = vec![added_row(ctx, &sheet.added)];
  rows.extend(reaches_row(ctx, &sheet.reaches));
  rows.push(rows::row(
    "team-hub-from-team-when",
    &ctx.t("team_hub.plain.when"),
    None,
    rows::words("team-hub-from-team-when-value", &when(ctx, &sheet.events)),
  ));
  rows.push(used_by_row(ctx, &sheet.used_by));
  rows.push(rows::row(
    "team-hub-from-team-format",
    &ctx.t("team_hub.plain.format"),
    None,
    rows::words("team-hub-from-team-format-value", &ctx.t("team_hub.plain.markdown")),
  ));
  vec![from_team(ctx, rows).into(), text(ctx, &sheet.text).into()]
}

/// *Roles* or *Profiles* in tags; nothing for an instruction every agent gets.
fn reaches_row(ctx: &Ctx, reaches: &Reaches) -> Option<lurq::components::Row> {
  let (label, names): (_, Vec<String>) = match reaches {
    Reaches::Everyone => return None,
    Reaches::Roles(roles) => (ctx.t("team_hub.plain.roles"), roles.clone()),
    Reaches::Profiles(profiles) => (
      ctx.t("team_hub.plain.profiles"),
      profiles.iter().map(|profile| profile.name.clone()).collect(),
    ),
  };
  Some(rows::row(
    "team-hub-from-team-reaches",
    &label,
    None,
    rows::tags("team-hub-from-team-reaches-value", names),
  ))
}

fn when(ctx: &Ctx, events: &[InjectionEvent]) -> String {
  let key = match events {
    [InjectionEvent::Start] => "team_hub.plain.whenStart",
    [InjectionEvent::Resume] => "team_hub.plain.whenResume",
    _ => "team_hub.plain.whenBoth",
  };
  ctx.t(key).to_string()
}

/// *Text*: the instruction as an agent reads it, in a box of its own (`Text` `jHGZ4`).
fn text(ctx: &Ctx, text: &str) -> Column {
  Column::new()
    .id("team-hub-text")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&ctx.t("team_hub.plain.text"))
        .id("team-hub-text-label")
        .variant(HubText::Kind.role())
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(
      Column::new()
        .id("team-hub-text-area")
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::Row.role()))
        .background(PaletteColor::SurfaceInput)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Md)
        .child(
          Text::new(text)
            .id("team-hub-text-value")
            .variant(HubText::Area.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
}
