use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Rect, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::connection::{
  AgentProvider,
  plan_limits::{LimitBlock, LimitState, LimitWindow},
};

use super::{Entry, metadata, text};
use crate::{
  messages::Message,
  theme::{
    ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, PLAN_HINT_WIDTH, PLAN_METER_HEIGHT, PLAN_METER_WIDTH,
    PLAN_PERCENT_WIDTH, PLAN_WINDOW_LABEL,
  },
  ui::message_text,
};

pub fn panel(ctx: &Ctx, entries: &[Entry<'_>]) -> Element {
  let mut panel = frame();
  for (index, entry) in entries.iter().enumerate() {
    if index > 0 {
      panel = panel.child(divider(ctx));
    }
    panel = panel.child(section(ctx, entry));
  }
  with_footnote(ctx, panel)
}

/// The hint of an item whose project connections could not be read.
pub fn unreadable(ctx: &Ctx, error: &Message) -> Element {
  let section = Column::new()
    .spacing(ExtraSpacing::PlanRows.role())
    .child(Text::new(&ctx.t("statusBar.planLimits.unreadable")).variant(TypographyStyle::Caption))
    .child(
      Text::new(&message_text(ctx, error))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextSecondary),
    );
  with_footnote(ctx, frame().child(section))
}

fn frame() -> Column {
  Column::new()
    .id("status-plan-limits-hint")
    .width(Dimension::Px(PLAN_HINT_WIDTH))
    .padding(SpacingSize::Sm)
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
}

fn with_footnote(ctx: &Ctx, panel: Column) -> Element {
  panel
    .child(divider(ctx))
    .child(
      Text::new(&ctx.t("statusBar.planLimits.footnote"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    )
    .into()
}

/// A hairline as thick as the theme's thin border.
fn divider(ctx: &Ctx) -> Element {
  let thickness = ctx.theme().border_size_value(BorderSize::Sm);
  Rect::new(Dimension::Pct(100.0), Dimension::Px(thickness))
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
  if entry.provider == AgentProvider::ClaudeCodeCli && windows.contains(&10080) {
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
  if entry.state() == LimitState::Reached {
    section = section.child(
      Text::new(&refuses(ctx, entry))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary),
    );
  }
  metadata::sources(ctx, entry, metadata::account_rows(ctx, entry, section)).into()
}

/// What the CLI does while the limit holds, and until when when it says.
fn refuses(ctx: &Ctx, entry: &Entry<'_>) -> std::sync::Arc<str> {
  let cli = text::name(ctx, entry.provider);
  let key = match entry.block_beyond_reset() {
    Some(LimitBlock::Credits) => "statusBar.planLimits.refusesCredits",
    Some(LimitBlock::Workspace) => "statusBar.planLimits.refusesWorkspace",
    Some(LimitBlock::Window | LimitBlock::Other) => "statusBar.planLimits.refusesOther",
    None => match entry.window().and_then(LimitWindow::resets) {
      Some(at) => {
        return ctx.t_args(
          "statusBar.planLimits.refuses",
          [("cli", &*cli), ("time", text::clock(at).as_str())],
        );
      }
      None => "statusBar.planLimits.refusesOther",
    },
  };
  ctx.t_args(key, [("cli", &*cli)])
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
    let at = window.resets().map(text::clock).unwrap_or_default();
    return row
      .child(
        Text::new(&ctx.t_args("statusBar.planLimits.expired", [("time", at.as_str())]))
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextMuted),
      )
      .into();
  }
  let state = entry.reading.map_or_else(
    || window.state(entry.now),
    |reading| reading.window_state(window, entry.now),
  );
  let color = if state == LimitState::Normal {
    PaletteColor::TextSecondary
  } else {
    PaletteColor::TextPrimary
  };
  // A window reported without its use shows an empty meter and no figure.
  let used = window.percent();
  let meter = Row::new()
    .width(Dimension::Px(PLAN_METER_WIDTH))
    .height(Dimension::Px(PLAN_METER_HEIGHT))
    .background(PaletteColor::Border)
    .rounded(ExtraRadius::Mark.role())
    .with_children(used.map(|used| {
      Rect::new(Dimension::Pct(used as f32), Dimension::Pct(100.0))
        .background(color)
        .rounded(ExtraRadius::Mark.role())
    }));
  let variant = if state == LimitState::Normal {
    ExtraTypography::MonoValue
  } else {
    ExtraTypography::MonoLimit
  };
  let reset_color = if state == LimitState::Normal {
    PaletteColor::TextMuted
  } else {
    PaletteColor::TextPrimary
  };
  let figure = Row::new()
    .id("status-plan-limits-percent")
    .width(Dimension::Px(PLAN_PERCENT_WIDTH))
    .justify(Justify::End)
    .child(
      Text::new(&used.map_or_else(|| ctx.t("statusBar.counts.unknown"), |used| text::percent(ctx, used)))
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
