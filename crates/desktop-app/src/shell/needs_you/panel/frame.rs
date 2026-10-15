//! The panel's head (`mxdKm`), what it shows when nothing waits (`oMseR`) and its footer (`mG6gL`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::{Alignment, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;

use super::{Group, leads::go};
use crate::{
  navigation::{Navigation, waiting::Destination},
  routes::{OverviewTab, ProjectPage, Route},
  theme::{ExtraTypography, IconSize, NeedsYouSpacing, NeedsYouText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    icon_button::IconButton,
  },
};

/// The measure of what the empty panel explains (`RAYK9` in `B95uP1`).
const EMPTY_TEXT_WIDTH: f32 = 320.0;

/// The head: the hand, *Needs you* and how many wait, the close, and the projects under them.
pub(super) fn head(ctx: &mut Ctx, total: usize, sub: &str, open: &Signal<bool>) -> Element {
  let color = if total > 0 {
    PaletteColor::Warning
  } else {
    PaletteColor::TextMuted
  };
  let close = {
    let open = open.clone();
    IconButton::new("needs-you-close", Icon::X, ctx.t("needs_you.close"))
      .inspector()
      .on_click(move || open.set(false))
      .render(ctx)
  };
  Column::new()
    .id("needs-you-head")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(crate::theme::ExtraSpacing::Loose.role())
        .bottom(crate::theme::ExtraSpacing::Loose.role())
        .right(SpacingSize::Sm)
        .left(SpacingSize::Lg),
    )
    .spacing(crate::theme::ExtraSpacing::Tight.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(NeedsYouSpacing::TitleRow.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Hand, IconSize::Nav, color.clone()))
        .child(
          Text::new(&ctx.t("needs_you.title"))
            .id("needs-you-title")
            .variant(NeedsYouText::Title.role()),
        )
        .child(
          Text::new(&total.to_string())
            .id("needs-you-count")
            .variant(NeedsYouText::Count.role())
            .color(color),
        )
        .child(Spacer::new().flex(1.0))
        .child(close),
    )
    .child(Text::new(sub).id("needs-you-sub").variant(ExtraTypography::Note.role()))
    .into()
}

/// Nothing waits (`oMseR`): the hand, *Nothing needs you* and what appears here.
pub(super) fn empty(ctx: &Ctx) -> Element {
  Column::new()
    .id("needs-you-empty")
    .width(Dimension::Pct(100.0))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .padding(
      Padding::new()
        .top(SpacingSize::Xs)
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .bottom(SpacingSize::Sm),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(
          NeedsYouSpacing::EmptySides.role(),
          crate::theme::ExtraSpacing::Roomy.role(),
        ))
        .spacing(crate::theme::ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Hand, IconSize::Xl, PaletteColor::TextMuted))
        .child(
          Text::new(&ctx.t("needs_you.empty.title"))
            .id("needs-you-empty-title")
            .variant(ExtraTypography::Strong.role()),
        )
        .child(
          Text::new(&ctx.t("needs_you.empty.text"))
            .id("needs-you-empty-text")
            .variant(NeedsYouText::Empty.role())
            .text_align(TextAlign::Center)
            .width(Dimension::Px(EMPTY_TEXT_WIDTH)),
        ),
    )
    .into()
}

/// *Show all in Kontur's Overview*, or with nothing listed *Open Kontur's Overview* (`k4VANT` in `B95uP1`), of
/// the active tab's project or the first listed, and *Esc closes*.
pub(super) fn footer(
  ctx: &Ctx,
  groups: &[Group],
  active: Option<ProjectId>,
  (navigation, open): (&Navigation, &Signal<bool>),
) -> Element {
  let chosen = groups
    .iter()
    .find(|group| Some(group.waiting.project) == active)
    .or_else(|| groups.first())
    .map(|group| &group.waiting);
  let key = if groups.iter().all(|group| group.items.is_empty()) {
    "needs_you.openOverview"
  } else {
    "needs_you.showAll"
  };
  let show_all = chosen.map(|waiting| {
    let route = Route::Project(waiting.project, ProjectPage::Overview(OverviewTab::NeedsYou));
    let press = go(navigation, open, Destination { route, pointed: None });
    ActionButton::new(
      "needs-you-show-all",
      ButtonKind::Ghost,
      ctx.t_args(key, [("project", waiting.name.as_str())]),
    )
    .icon(Icon::LayoutDashboard)
    .on_click(move || press())
  });
  Row::new()
    .id("needs-you-footer")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(crate::theme::ExtraSpacing::Snug.role())
        .bottom(crate::theme::ExtraSpacing::Snug.role())
        .right(SpacingSize::Md)
        .left(SpacingSize::Sm),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(show_all)
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&ctx.t("needs_you.escCloses"))
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
    .into()
}
