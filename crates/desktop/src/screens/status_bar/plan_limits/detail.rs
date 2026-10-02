use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Rect, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::connection::plan_limits::{LimitState, LimitWindow};

use super::{Cli, Entry, metadata, text};
use crate::theme::{
  ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, PLAN_HAIRLINE, PLAN_HINT_WIDTH, PLAN_METER_HEIGHT,
  PLAN_METER_WIDTH, PLAN_WINDOW_LABEL,
};

pub fn panel(ctx: &Ctx, entries: &[Entry<'_>]) -> Element {
  let mut panel = Column::new()
    .id("status-plan-limits-hint")
    .width(Dimension::Px(PLAN_HINT_WIDTH))
    .padding(SpacingSize::Sm)
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md);
  for (index, entry) in entries.iter().enumerate() {
    if index > 0 {
      panel = panel.child(divider());
    }
    panel = panel.child(section(ctx, entry));
  }
  panel
    .child(divider())
    .child(
      Text::new(&ctx.t("statusBar.planLimits.footnote"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    )
    .into()
}

fn divider() -> Element {
  Rect::new(Dimension::Pct(100.0), Dimension::Px(PLAN_HAIRLINE))
    .background(ExtraColor::Hairline.role())
    .into()
}

fn section(ctx: &Ctx, entry: &Entry<'_>) -> Element {
  let mut section = Column::new()
    .spacing(ExtraSpacing::PlanRows.role())
    .child(metadata::header(ctx, entry));
  let Some(reading) = entry.reading else {
    return section.child(metadata::unavailable(ctx, entry)).into();
  };
  let mut windows: Vec<_> = reading.limits.windows.iter().map(|window| window.minutes()).collect();
  // The weekly Claude report leaves its expected five-hour window unknown (`mFTm5`).
  // A five-hour-only or Codex report uses only the rows actually reported (`b3WD6`).
  if entry.cli == Cli::Claude && windows.contains(&10080) {
    windows.push(300);
  }
  windows.sort_unstable();
  windows.dedup();
  for minutes in windows {
    let row = reading
      .limits
      .windows
      .iter()
      .find(|window| window.minutes() == minutes)
      .map_or_else(|| unknown_window(ctx, minutes), |window| window_row(ctx, entry, window));
    section = section.child(row);
  }
  if let Some(window) = entry.window().filter(|_| entry.state() == LimitState::Reached) {
    let at = window
      .resets()
      .map(|at| text::clock(at, false))
      .unwrap_or_else(|| ctx.t("statusBar.planLimits.resetUnknown").to_string());
    section = section.child(
      Text::new(&ctx.t_args(
        "statusBar.planLimits.refuses",
        [("cli", &*text::name(ctx, entry.cli)), ("time", at.as_str())],
      ))
      .variant(TypographyStyle::Caption),
    );
  }
  metadata::sources(ctx, entry, metadata::account_rows(ctx, entry, section)).into()
}

fn unknown_window(ctx: &Ctx, minutes: u64) -> Element {
  Row::new()
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&text::window(ctx, minutes))
        .variant(TypographyStyle::Caption)
        .width(Dimension::Px(PLAN_WINDOW_LABEL)),
    )
    .child(
      Text::new(&ctx.t("statusBar.planLimits.notReported"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    )
    .into()
}

fn window_row(ctx: &Ctx, entry: &Entry<'_>, window: &LimitWindow) -> Element {
  let label = Text::new(&text::window(ctx, window.minutes()))
    .variant(TypographyStyle::Caption)
    .width(Dimension::Px(PLAN_WINDOW_LABEL));
  let row = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(label);
  if window.expired(entry.now) {
    let at = window.resets().map(|at| text::clock(at, false)).unwrap_or_default();
    return row
      .child(
        Text::new(&ctx.t_args("statusBar.planLimits.expired", [("time", at.as_str())]))
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextMuted),
      )
      .into();
  }
  let color = if window.state(entry.now) == LimitState::Normal {
    PaletteColor::TextSecondary
  } else {
    PaletteColor::TextPrimary
  };
  let meter = Row::new()
    .width(Dimension::Px(PLAN_METER_WIDTH))
    .height(Dimension::Px(PLAN_METER_HEIGHT))
    .background(PaletteColor::Border)
    .rounded(ExtraRadius::Mark.role())
    .child(
      Rect::new(Dimension::Pct(window.percent() as f32), Dimension::Pct(100.0))
        .background(color)
        .rounded(ExtraRadius::Mark.role()),
    );
  let variant = if window.state(entry.now) == LimitState::Normal {
    ExtraTypography::MonoValue
  } else {
    ExtraTypography::MonoLimit
  };
  let reset_color = if window.state(entry.now) == LimitState::Normal {
    PaletteColor::TextMuted
  } else {
    PaletteColor::TextPrimary
  };
  let figure = Row::new().justify(Justify::End).child(
    Text::new(&text::percent(ctx, window.percent()))
      .variant(variant.role())
      .color(PaletteColor::TextPrimary),
  );
  row
    .child(meter)
    .child(figure)
    .child(
      Text::new(&text::reset(ctx, window.resets(), entry.now, true))
        .flex(1.0)
        .variant(ExtraTypography::MonoValue.role())
        .color(reset_color),
    )
    .into()
}
