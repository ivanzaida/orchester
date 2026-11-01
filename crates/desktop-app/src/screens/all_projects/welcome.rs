//! The first start (`docs/design/startup.md` decisions 1–3; `Startup · redesign` `qzzYw`: 01 `H9CZ1`, in light 01L
//! `ifctO`, signed in to Orchester Cloud 01b `Knacr`), while a session has no project yet: the mark, *WELCOME TO
//! ORCHESTER*, *How do you want to start?* and what Orchester does, the three equal ways in and the line that says no
//! account is needed ([`ways`]), and a footer that says what stays on this computer. The ways lead to the start page's
//! steps in place, each in the step frame of *Join a team* (`crate::screens::join_team::frame`): its breadcrumb back
//! to *Start*, the step line and *Back*. That an agent CLI is not found shows under the ways ([`super::agent_cli`]).
//! Nothing is made up: no recent project, no activity, no runner.

mod card;
mod ways;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{folder::OpenAction, screen_id};
use crate::{
  brand,
  theme::{IconSize, ProjectsSpacing, ProjectsText, StartupSpacing, StartupText},
  ui::icon::{Icon, icon},
};

/// The column of the head, the ways and the line under them (`Inner` `nRGvM`), and the lead's width (`PASNQ`).
const INNER_WIDTH: f32 = 1040.0;
const LEAD_WIDTH: f32 = 640.0;
/// The footer (`Footer` `JSWOZ`).
const FOOTER_HEIGHT: f32 = 56.0;

/// The first start, with that an agent CLI is not found under its ways when `cli` says so; *Choose folder…* runs
/// `open`.
pub(super) fn welcome(ctx: &mut Ctx, open: &OpenAction, cli: Option<Element>) -> Element {
  let inner = Column::new()
    .id("welcome-inner")
    .width(Dimension::Pct(100.0))
    .max_width(Dimension::Px(INNER_WIDTH))
    .spacing(SpacingSize::Section)
    .child(head(ctx))
    .child(ways::ways(ctx, open));
  let main = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ProjectsSpacing::WelcomeEdge.role(),
      ProjectsSpacing::Wide.role(),
    ))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(inner);
  let main: Element = match cli {
    None => main.flex(1.0).into(),
    // It does not fit at the window's smallest size with the warning, so only then the page scrolls; as high as it is
    // when it fits, centred in the panel.
    Some(cli) => Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .justify(Justify::Center)
      .child(
        ScrollVertical::new(
          main.spacing(SpacingSize::Section).child(
            Column::new()
              .width(Dimension::Pct(100.0))
              .max_width(Dimension::Px(INNER_WIDTH))
              .child(cli),
          ),
        )
        .id("welcome-scroll")
        .width(Dimension::Pct(100.0))
        .max_height(Dimension::Pct(100.0)),
      )
      .into(),
  };
  Column::new()
    .id(screen_id())
    .describe("state", "welcome")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(main)
    .child(footer(ctx))
    .into()
}

/// The mark, the eyebrow, the question and what Orchester does (`Head` `A03a5`).
fn head(ctx: &Ctx) -> Column {
  Column::new()
    .id("welcome-head")
    .width(Dimension::Pct(100.0))
    .spacing(StartupSpacing::Head.role())
    .child(brand::mark())
    .child(
      Text::new(&ctx.t("startup.start.eyebrow"))
        .variant(ProjectsText::Eyebrow.role())
        .nowrap(),
    )
    .child(
      Text::new(&ctx.t("startup.start.title"))
        .id("welcome-title")
        .variant(StartupText::StartTitle.role()),
    )
    .child(
      Text::new(&ctx.t("startup.start.lead"))
        .id("welcome-lead")
        .variant(StartupText::Lead.role())
        .width(Dimension::Pct(100.0))
        .max_width(Dimension::Px(LEAD_WIDTH)),
    )
}

/// What stays on this computer and what a team server keeps (`Footer` `JSWOZ`).
fn footer(ctx: &Ctx) -> Row {
  Row::new()
    .id("welcome-footer")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Xl))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::HardDrive, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("startup.start.footer"))
        .variant(StartupText::Footer.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}
