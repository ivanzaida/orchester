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
  ConnectionStatus,
  plan_limits::{LimitState, LimitWindow},
};

use super::{Entry, text};
use crate::{
  display,
  theme::{ExtraColor, ExtraRadius, ExtraTypography},
  ui::message_text,
};

// poPso hint mFTm5: 360 wide; window labels 44; meter 100 × 4.
const WIDTH: f32 = 360.0;
const LABEL: f32 = 44.0;
const METER_WIDTH: f32 = 100.0;
const METER_HEIGHT: f32 = 4.0;
const HAIRLINE: f32 = 1.0;

pub fn panel(ctx: &Ctx, entries: &[Entry<'_>]) -> Element {
  let mut panel = Column::new()
    .id("status-plan-limits-hint")
    .width(Dimension::Px(WIDTH))
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
  Rect::new(Dimension::Pct(100.0), Dimension::Px(HAIRLINE))
    .background(ExtraColor::Hairline.role())
    .into()
}

fn section(ctx: &Ctx, entry: &Entry<'_>) -> Element {
  let name = text::name(ctx, entry.provider);
  let mut header = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&name).variant(ExtraTypography::StatusLabel.role()));
  if let Some(plan) = entry.reading.and_then(|reading| reading.limits.plan.as_deref()) {
    header = header.child(Text::new(plan).variant(TypographyStyle::Caption));
  }
  let mut section = Column::new().spacing(SpacingSize::Sm).child(header);
  let Some(reading) = entry.reading else {
    return section.child(unavailable(ctx, entry)).into();
  };
  if let Some(window) = reading
    .limiting(entry.now)
    .filter(|_| entry.state() != LimitState::Normal)
  {
    let key = if entry.state() == LimitState::Reached {
      "statusBar.planLimits.windowReached"
    } else {
      match window.minutes() {
        300 => "statusBar.planLimits.nearFiveHour",
        10080 => "statusBar.planLimits.nearWeekly",
        _ => "statusBar.planLimits.windowNear",
      }
    };
    section = section.child(
      Text::new(&ctx.t_args(key, [("window", &*text::window(ctx, window.minutes()))]))
        .variant(ExtraTypography::StatusLabel.role()),
    );
  }
  let mut windows: Vec<_> = reading.limits.windows.iter().collect();
  windows.sort_by_key(|window| window.minutes());
  for window in windows {
    section = section.child(window_row(ctx, entry, window));
  }
  for minutes in [300, 10080] {
    if !reading.limits.windows.iter().any(|window| window.minutes() == minutes) {
      section = section.child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .child(
            Text::new(&text::window(ctx, minutes))
              .variant(TypographyStyle::Caption)
              .width(Dimension::Px(LABEL)),
          )
          .child(
            Text::new(&ctx.t("statusBar.planLimits.notReported"))
              .variant(TypographyStyle::Caption)
              .color(PaletteColor::TextMuted),
          ),
      );
    }
  }
  if let Some(window) = entry.window().filter(|_| entry.state() == LimitState::Reached) {
    let at = window
      .resets()
      .map(|at| text::clock(at, false))
      .unwrap_or_else(|| ctx.t("statusBar.planLimits.resetUnknown").to_string());
    section = section.child(
      Text::new(&ctx.t_args("statusBar.planLimits.refuses", [("cli", &*name), ("time", at.as_str())]))
        .variant(TypographyStyle::Caption),
    );
  }
  section = account_rows(ctx, entry, section);
  let age = message_text(ctx, &display::elapsed(entry.now, reading.at));
  section
    .child(
      Text::new(&ctx.t_args(
        "statusBar.planLimits.source",
        [
          ("cli", &*name),
          ("time", text::clock(reading.at, false).as_str()),
          ("age", &*age),
        ],
      ))
      .variant(TypographyStyle::Caption)
      .color(PaletteColor::TextMuted),
    )
    .into()
}

fn account_rows(ctx: &Ctx, entry: &Entry<'_>, mut section: Column) -> Column {
  let Some(reading) = entry.reading else {
    return section;
  };
  if reading
    .limits
    .windows
    .iter()
    .any(|window| window.extra() && !window.expired(entry.now))
  {
    section = section.child(Text::new(&ctx.t("statusBar.planLimits.extraUsage")).variant(TypographyStyle::Caption));
  }
  if let Some(credits) = reading
    .limits
    .credits
    .as_ref()
    .filter(|credits| credits.has_credits || credits.unlimited)
  {
    let text = if credits.unlimited {
      ctx.t("statusBar.planLimits.creditsUnlimited")
    } else {
      let balance = credits
        .balance
        .as_deref()
        .map_or_else(|| ctx.t("statusBar.counts.unknown"), Into::into);
      ctx.t_args("statusBar.planLimits.credits", [("balance", &*balance)])
    };
    section = section.child(Text::new(&text).variant(TypographyStyle::Caption));
  }
  section
}

fn unavailable(ctx: &Ctx, entry: &Entry<'_>) -> Element {
  let key = match entry.connection.status() {
    ConnectionStatus::SignInRequired => "statusBar.planLimits.signedOut",
    ConnectionStatus::ExecutableMissing | ConnectionStatus::CouldNotRun | ConnectionStatus::NotRecognized => {
      "statusBar.planLimits.cannotStart"
    }
    _ => "statusBar.planLimits.notRead",
  };
  let mut column = Column::new()
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t_args(key, [("cli", &*text::name(ctx, entry.provider))])).variant(TypographyStyle::Caption));
  if let Some(check) = entry.connection.last_check() {
    column = column.child(
      Text::new(&ctx.t_args(
        "statusBar.planLimits.checked",
        [("time", text::clock(check.at, false).as_str())],
      ))
      .variant(TypographyStyle::Caption)
      .color(PaletteColor::TextMuted),
    );
  }
  column.into()
}

fn window_row(ctx: &Ctx, entry: &Entry<'_>, window: &LimitWindow) -> Element {
  let label = Text::new(&text::window(ctx, window.minutes()))
    .variant(TypographyStyle::Caption)
    .width(Dimension::Px(LABEL));
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
    .width(Dimension::Px(METER_WIDTH))
    .height(Dimension::Px(METER_HEIGHT))
    .background(PaletteColor::Border)
    .rounded(ExtraRadius::Mark.role())
    .child(
      Rect::new(Dimension::Pct(window.percent() as f32), Dimension::Pct(100.0))
        .background(color.clone())
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
      .color(color),
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
