//! The setup rail (`Setup rail` `l9ZCVs`): *PROJECT SETUP*, the step under way, the two steps this version
//! cannot do yet with why, and at the bottom Back with what leaving does. Back returns to where Add project was
//! opened, or to All projects when nothing was before it.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  navigation::Navigation,
  routes::Route,
  theme::{ExtraColor, ExtraSpacing, IconSize, ProjectsSpacing, ProjectsText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// The rail's width (`l9ZCVs`) and the slot that keeps a step's icon with its label (`Icon slot` `YD25L`).
const RAIL_WIDTH: f32 = 260.0;
const ICON_SLOT: f32 = 18.0;

pub(super) fn rail(ctx: &mut Ctx) -> Element {
  let navigation = Navigation::of(ctx);
  let back = ActionButton::new("add-project-back", ButtonKind::Secondary, ctx.t("setup.rail.back"))
    .icon(Icon::ArrowLeft)
    .fill_width()
    .on_click(move || {
      if navigation.tabs_now().active().can_back() {
        navigation.back();
      } else {
        navigation.go(Route::AllProjects);
      }
    });
  let unavailable = ctx.t("setup.rail.notAvailable");
  Column::new()
    .id("add-project-rail")
    .width(Dimension::Px(RAIL_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Xl)
        .right(ExtraSpacing::Inset.role())
        .bottom(ProjectsSpacing::Page.role())
        .left(ExtraSpacing::Inset.role()),
    )
    .spacing(SpacingSize::Xs)
    .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .padding(
          Padding::new()
            .right(ExtraSpacing::Loose.role())
            .bottom(ExtraSpacing::Loose.role())
            .left(ExtraSpacing::Loose.role()),
        )
        .child(
          Text::new(&ctx.t("setup.rail.heading"))
            .variant(ProjectsText::Eyebrow.role())
            .nowrap(),
        ),
    )
    .child(step("add-project-step-project", &ctx.t("setup.rail.project"), None))
    .child(step(
      "add-project-step-agents",
      &ctx.t("setup.rail.agents"),
      Some(&unavailable),
    ))
    .child(step(
      "add-project-step-ready",
      &ctx.t("setup.rail.ready"),
      Some(&unavailable),
    ))
    .child(Spacer::new().flex(1.0))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::horizontal(SpacingSize::Xs))
        .spacing(SpacingSize::Sm)
        .child(back)
        .child(Text::new(&ctx.t("setup.rail.backHint")).variant(TypographyStyle::Caption)),
    )
    .into()
}

/// A step: the one under way on the `hover` fill (`Step 1` `ZQiHG`), or one this version cannot do yet, locked
/// with why under its label (`Step 2` `asMZR`).
fn step(id: &str, label: &str, unavailable: Option<&str>) -> Row {
  let (glyph, mark, style) = match unavailable {
    None => (Icon::CircleDot, PaletteColor::TextPrimary, ProjectsText::Step),
    Some(_) => (Icon::Lock, PaletteColor::TextMuted, ProjectsText::StepLocked),
  };
  let glyph_size = if unavailable.is_some() {
    IconSize::Base
  } else {
    IconSize::Md
  };
  let row = Row::new()
    .id(id.to_owned())
    .describe(
      "state",
      if unavailable.is_some() {
        "unavailable"
      } else {
        "current"
      },
    )
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Loose.role(),
      ExtraSpacing::TextArea.role(),
    ))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .rounded(RadiusSize::Md)
    .child(
      Row::new()
        .height(Dimension::Px(ICON_SLOT))
        .align_items(Alignment::Center)
        .child(icon(glyph, glyph_size, mark)),
    )
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(label).variant(style.role()))
        .with_children(unavailable.map(|why| Text::new(why).variant(TypographyStyle::Caption))),
    );
  if unavailable.is_some() {
    row
  } else {
    row.background(ExtraColor::Hover.role())
  }
}
