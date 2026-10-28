//! The usage card at the sidebar's bottom (`Usage card` `sAhNt` in `Zn8dR`): *Tokens today* with today's tokens
//! against the project's daily budget (`244k / 20M`, a dash until they are read) and the budget's meter, which
//! fills in `warning` once the budget is reached and shows only with a budget. A press on it opens the project's
//! Usage; the ⚙ beside it opens Settings. The status bar no longer shows the tokens, so they show once.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, tokens::compact_tokens};

use crate::{
  navigation::Navigation,
  project::Projects,
  routes::{ProjectPage, Route},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    icon::Icon,
    icon_button::IconButton,
    meter::{MeterTone, meter},
    pressable::opening,
  },
};

/// The card of `project`.
pub(super) fn card(ctx: &mut Ctx, project: ProjectId, navigation: &Navigation) -> Element {
  let usage = Projects::of(ctx).feed(project).usage.get();
  let unknown = ctx.t("shell.sidebar.unknownCount").to_string();
  let (used, budget) = usage.as_ref().map_or_else(
    || (unknown, None),
    |usage| {
      let used = usage.today.get();
      let budget = usage.limits.daily_budget_limit().map(|budget| budget.get());
      (compact_tokens(used), budget.map(|budget| (used, budget)))
    },
  );
  let value = match budget {
    Some((_, budget)) => ctx.t_args(
      "shell.usage.value",
      [("used", used), ("budget", compact_tokens(budget))],
    ),
    None if usage.is_some() => ctx.t_args(
      "shell.usage.value",
      [
        ("used", used),
        ("budget", ctx.t("project_settings.noLimit").to_string()),
      ],
    ),
    None => used.into(),
  };
  let line = Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("shell.usage.today"))
        .variant(TypographyStyle::Caption)
        .nowrap()
        .flex(1.0),
    )
    .child(
      Text::new(&value)
        .id("sidebar-usage-value")
        .variant(ExtraTypography::MonoNote.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    );
  let meter = budget.map(|(used, budget)| {
    let tone = if used >= budget {
      MeterTone::Reached
    } else {
      MeterTone::Plain
    };
    meter("sidebar-usage-meter", (used, budget), tone)
  });
  let usage = opening(navigation.to(Route::Project(project, ProjectPage::Usage(Box::default()))))
    .id("sidebar-usage")
    .describe("a11y_name", ctx.t("shell.usage.open"))
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(line)
        .with_children(meter),
    );
  let settings = IconButton::new("sidebar-settings", Icon::Settings, ctx.t("shell.screens.settings"))
    .inspector()
    .opens(navigation.to(Route::Settings))
    .render(ctx);
  Row::new()
    .id("sidebar-usage-card")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()).right(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .rounded(ExtraRadius::Tile.role())
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .align_items(Alignment::Center)
    .child(usage)
    .child(settings)
    .into()
}
