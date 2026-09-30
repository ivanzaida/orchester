//! Provider headings, account notes and honest per-window provenance in `poPso` hints.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::Element,
};
use orchester_core::{
  connection::{AgentProvider, ConnectionStatus, plan_limits::LimitState},
  time::Timestamp,
};

use super::{Entry, text};
use crate::{
  display,
  theme::{ExtraSpacing, ExtraTypography},
  ui::message_text,
};

pub fn header(ctx: &Ctx, entry: &Entry<'_>) -> Element {
  let mut row = Row::new()
    .id(match entry.provider {
      AgentProvider::CodexCli => "status-plan-limits-codex-heading",
      AgentProvider::ClaudeCodeCli => "status-plan-limits-claude-heading",
    })
    .spacing(ExtraSpacing::PlanRows.role())
    .align_items(Alignment::Center)
    .child(Text::new(&text::hint_name(ctx, entry.provider)).variant(ExtraTypography::StatusLabel.role()));
  if let Some(plan) = entry.reading.and_then(|reading| reading.limits.plan.as_deref()) {
    row = row.child(Text::new(&text::plan(ctx, plan)).variant(TypographyStyle::Caption));
  }
  let state = heading_state(ctx, entry);
  if let Some(state) = state {
    row = row.child(
      Text::new(&state)
        .variant(TypographyStyle::Caption)
        .color(if entry.reading.is_some() {
          PaletteColor::TextPrimary
        } else {
          PaletteColor::TextSecondary
        }),
    );
  }
  row.into()
}

fn heading_state(ctx: &Ctx, entry: &Entry<'_>) -> Option<std::sync::Arc<str>> {
  let Some(reading) = entry.reading else {
    return Some(ctx.t(match entry.connection.status() {
      ConnectionStatus::SignInRequired => "statusBar.planLimits.stateSignedOut",
      ConnectionStatus::ExecutableMissing | ConnectionStatus::CouldNotRun | ConnectionStatus::NotRecognized => {
        "statusBar.planLimits.stateUnavailable"
      }
      _ => "statusBar.planLimits.stateNotRead",
    }));
  };
  let window = reading.limiting(entry.now)?;
  // Extra usage has its own explanation row, without a second warning heading (`b3WD6`).
  if entry.state() == LimitState::Normal || window.extra() {
    return None;
  }
  let key = match entry.state() {
    LimitState::Reached => "statusBar.planLimits.headerReached",
    _ => match window.minutes() {
      300 => "statusBar.planLimits.nearFiveHour",
      10080 => "statusBar.planLimits.nearWeekly",
      _ => "statusBar.planLimits.windowNear",
    },
  };
  Some(ctx.t_args(key, [("window", &*text::window(ctx, window.minutes()))]))
}

pub fn unavailable(ctx: &Ctx, entry: &Entry<'_>) -> Element {
  let failed = matches!(
    entry.connection.status(),
    ConnectionStatus::ExecutableMissing | ConnectionStatus::CouldNotRun | ConnectionStatus::NotRecognized
  );
  let key = if failed {
    "statusBar.planLimits.cannotStart"
  } else if entry.connection.status() == ConnectionStatus::SignInRequired {
    "statusBar.planLimits.signedOut"
  } else {
    "statusBar.planLimits.notRead"
  };
  let checked = entry
    .connection
    .last_check()
    .map(|check| text::clock(check.at, false))
    .unwrap_or_else(|| ctx.t("statusBar.planLimits.checkUnknown").to_string());
  let body = ctx.t_args(
    key,
    [
      ("cli", &*text::hint_name(ctx, entry.provider)),
      ("connection", entry.connection.name().as_str()),
      ("project", entry.project_name),
      ("time", checked.as_str()),
    ],
  );
  let mut column = Column::new().spacing(ExtraSpacing::PlanRows.role()).child(
    Text::new(&body)
      .variant(TypographyStyle::Caption)
      .color(PaletteColor::TextSecondary),
  );
  if entry.connection.status() == ConnectionStatus::SignInRequired {
    column = column.child(
      Text::new(&ctx.t_args("statusBar.planLimits.checked", [("time", checked.as_str())]))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    );
  }
  column.into()
}

pub fn account_rows(ctx: &Ctx, entry: &Entry<'_>, mut section: Column) -> Column {
  let Some(reading) = entry.reading else {
    return section;
  };
  if let Some(credits) = reading
    .limits
    .credits
    .as_ref()
    .filter(|credits| credits.has_credits || credits.unlimited)
  {
    let value = if credits.unlimited {
      ctx.t("statusBar.planLimits.unlimited")
    } else {
      ctx.t_args(
        "statusBar.planLimits.creditsValue",
        [(
          "balance",
          credits.balance.as_deref().unwrap_or(&ctx.t("statusBar.counts.unknown")),
        )],
      )
    };
    section = section.child(account_row(ctx, "statusBar.planLimits.creditsLabel", &value));
  }
  if reading
    .limits
    .windows
    .iter()
    .any(|window| window.extra() && !window.expired(entry.now))
  {
    section = section.child(account_row(
      ctx,
      "statusBar.planLimits.extraLabel",
      &ctx.t("statusBar.planLimits.extraValue"),
    ));
  }
  section
}

fn account_row(ctx: &Ctx, key: &str, value: &str) -> Element {
  Row::new()
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&ctx.t(key))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextSecondary),
    )
    .child(
      Text::new(value)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary),
    )
    .into()
}

pub fn sources(ctx: &Ctx, entry: &Entry<'_>, mut section: Column) -> Column {
  let Some(reading) = entry.reading else {
    return section;
  };
  let mixed = reading
    .limits
    .windows
    .iter()
    .any(|window| window.reported_at().unwrap_or(reading.at) != reading.at);
  if mixed {
    let mut windows: Vec<_> = reading.limits.windows.iter().collect();
    windows.sort_by_key(|window| window.minutes());
    for window in windows {
      section = section.child(source(
        ctx,
        entry,
        window.reported_at().unwrap_or(reading.at),
        Some(window.minutes()),
      ));
    }
    section
  } else {
    section.child(source(ctx, entry, reading.at, None))
  }
}

fn source(ctx: &Ctx, entry: &Entry<'_>, at: Timestamp, minutes: Option<u64>) -> Element {
  let age = message_text(ctx, &display::elapsed(entry.now, at));
  let copy = ctx.t_args(
    "statusBar.planLimits.source",
    [
      ("cli", &*text::name(ctx, entry.provider)),
      ("time", text::clock(at, false).as_str()),
      ("age", &*age),
    ],
  );
  let copy = minutes.map_or_else(
    || copy.clone(),
    |minutes| {
      ctx.t_args(
        "statusBar.planLimits.windowSource",
        [("window", &*text::window(ctx, minutes)), ("source", &*copy)],
      )
    },
  );
  Text::new(&copy)
    .variant(TypographyStyle::Caption)
    .color(PaletteColor::TextMuted)
    .into()
}
