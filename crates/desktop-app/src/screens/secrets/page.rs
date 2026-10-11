//! The Secrets page's header, tabs and unrecorded access log.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::state::{Panel, View};
use crate::{
  project::ShownProject,
  theme::{ExtraSpacing, ExtraTypography, IconSize, SecretsSpacing, SecretsText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    inspector,
    state::EmptyState,
    tabs::{PageTab, counted, page_tabs},
  },
};

pub(super) fn frame() -> Column {
  Column::new()
    .id("screen-secrets")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Roomy.role())
        .bottom(SpacingSize::Xl)
        .left(SecretsSpacing::Page.role())
        .right(SecretsSpacing::Page.role()),
    )
    .spacing(SpacingSize::Lg)
}
pub(super) fn header(ctx: &Ctx, project: &ShownProject, view: &View) -> Row {
  let panel = view.panel.clone();
  let title = Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Snug.role())
    .child(Text::new(&ctx.t("secrets.title")).variant(ExtraTypography::Display.role()))
    .child(
      Text::new(&ctx.t_args(
        "secrets.description",
        [("project", project.open.project.name().as_str())],
      ))
      .variant(SecretsText::Lead.role()),
    );
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::End)
    .child(title)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .child(
          ActionButton::new("secrets-import", ButtonKind::Secondary, ctx.t("secrets.import"))
            .icon(Icon::Download)
            .unavailable(ctx.t("secrets.importUnavailable")),
        )
        .child(
          ActionButton::new("secrets-add", ButtonKind::Primary, ctx.t("secrets.add"))
            .icon(Icon::Plus)
            .on_click(move || panel.set(Panel::Add)),
        ),
    )
}
pub(super) fn tabs(ctx: &mut Ctx, view: &View, count: Option<usize>) -> Element {
  let usage = view.panel.get() == Panel::Usage;
  let (list, log) = (view.panel.clone(), view.panel.clone());
  page_tabs(
    ctx,
    "secrets-tabs",
    [
      PageTab::new(
        "secrets-tab-list",
        match count {
          None => ctx.t("secrets.listUnknown"),
          Some(0) => ctx.t("secrets.list"),
          Some(count) => counted(ctx, &ctx.t("secrets.list"), count),
        },
        !usage,
        move || list.set(Panel::Inspect),
      ),
      PageTab::new("secrets-tab-usage", ctx.t("secrets.usage"), usage, move || {
        log.set(Panel::Usage)
      }),
    ],
  )
  .into()
}
pub(super) fn usage(ctx: &Ctx) -> Element {
  let mut more = inspector::section("secrets-usage-details", ExtraSpacing::Snug.role())
    .child(Text::new(&ctx.t("secrets.usageHeading")).variant(SecretsText::UsageHeading.role()));
  for (id, glyph, key) in [
    ("secrets-usage-who", Icon::Bot, "secrets.usageWho"),
    ("secrets-usage-outcome", Icon::CircleCheck, "secrets.usageOutcome"),
    ("secrets-usage-never", Icon::LockKeyhole, "secrets.usageNever"),
  ] {
    more = more.child(
      Row::new()
        .id(id)
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
        .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Description)),
    );
  }
  EmptyState::new(
    "secrets-usage",
    Icon::ScrollText,
    ctx.t("secrets.usageTitle"),
    ctx.t("secrets.usageDetail"),
  )
  .secrets()
  .child(more)
  .into()
}
