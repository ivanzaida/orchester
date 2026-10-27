//! An account's *Plan limits* (`Plan limits` `iYa5d` in `03b · Account`): its windows as its CLI last reported them,
//! each a row with when it resets and a meter with the share used (a window at its limit in `warning`); when and how
//! the reading came in the group's heading, with a Claude Code account's *Check now* at its end; the CLI's note under
//! the group. Nothing is shown that the CLI did not report: without a reading the group says so.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Column, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  account::{AccountCli, AgentAccount},
  connection::plan_limits::{LimitReading, LimitWindow, ReadingSource},
  time::Timestamp,
};

use super::{
  PlanCheck, Work,
  words::{t, ta},
};
use crate::{
  services::plan_limits,
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
    meter::{MeterTone, meter},
    plan_window::{share, when},
    setting_rows::{HintTone, SettingGroup, SettingRow, group_note},
  },
};

/// A five-hour window and a weekly one, in minutes.
const FIVE_HOURS: u64 = 300;
const WEEK: u64 = 10_080;
/// A window's meter (`Meter` `Rfr8H`).
const METER_WIDTH: f32 = 140.0;

pub(super) fn group(ctx: &mut Ctx, work: &Work, account: &AgentAccount) -> Element {
  let now = Timestamp::now();
  let program = account.cli().program();
  let reading = last_reading(ctx, account);
  let windows: Vec<&LimitWindow> = reading
    .as_ref()
    .map(|(reading, _)| {
      reading
        .limits
        .windows
        .iter()
        .filter(|window| !window.expired(now))
        .collect()
    })
    .unwrap_or_default();
  let rows: Vec<Element> = match &reading {
    Some((reading, _)) => windows
      .iter()
      .map(|window| window_row(ctx, window, reading, now))
      .collect(),
    None => Vec::new(),
  };
  // Without a window reported, the group says so where its rows would be, then the CLI's note.
  let none = windows.is_empty().then(|| {
    let key = match account.cli() {
      AccountCli::ClaudeCode => "accounts.planLimits.none",
      AccountCli::Codex => "accounts.planLimits.noneCodex",
    };
    ta(ctx, key, [("program", program)])
  });
  let note_key = match account.cli() {
    AccountCli::ClaudeCode => "accounts.planLimits.noteClaude",
    AccountCli::Codex => "accounts.planLimits.noteCodex",
  };
  let cli_note = t(ctx, note_key);
  let mut group = SettingGroup::new("account-plan-limits", t(ctx, "accounts.planLimits.title")).rows(rows);
  let after = match none {
    Some(none) => {
      group = group.note(none);
      Some(group_note("account-plan-note", &cli_note))
    }
    None => {
      group = group.note(cli_note);
      None
    }
  };
  if let Some((reading, source)) = &reading {
    let at = when(ctx, reading.at, now, ("accounts.time.today", "accounts.time.day"));
    let key = match source {
      ReadingSource::Turn => "accounts.planLimits.metaTurn",
      ReadingSource::Read => "accounts.planLimits.metaRead",
    };
    group = group.meta(ta(ctx, key, [("time", at)]));
  }
  if account.cli() == AccountCli::ClaudeCode {
    group = group.end(check_now(ctx, work, account));
  }
  let failed = match work.plan_check_of(account.id()) {
    PlanCheck::Failed(error) => Some(field_error("account-plan-check-error", &message_text(ctx, &error))),
    PlanCheck::Idle | PlanCheck::Running => None,
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(group)
    .with_children(after)
    .with_children(failed)
    .into()
}

/// *Check now*, busy while it runs.
fn check_now(ctx: &Ctx, work: &Work, account: &AgentAccount) -> ActionButton {
  let button = ActionButton::new(
    "account-plan-check",
    ButtonKind::Ghost,
    t(ctx, "accounts.planLimits.checkNow"),
  )
  .icon(Icon::RefreshCw)
  .small();
  if work.plan_check_of(account.id()) == PlanCheck::Running {
    button.loading()
  } else {
    let (work, id) = (work.clone(), account.id());
    button.on_click(move || work.check_plan(id))
  }
}

/// The last reading kept of the account's plan limits, with where it came from, as read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stored(Option<(LimitReading, ReadingSource)>);

impl lurq::app::component::DevtoolsInspectable for Stored {}

/// The account's last plan reading and where it came from: this session's, or the last one kept (a project's turn or
/// a read), whichever is newer.
fn last_reading(ctx: &mut Ctx, account: &AgentAccount) -> Option<(LimitReading, ReadingSource)> {
  let id = account.id();
  let readings = plan_limits::readings(ctx);
  let session = readings.of_account(id).cloned().zip(readings.source_of(id));
  let workspace = crate::services::Services::of(ctx).workspace;
  let stored = ctx
    .future(id, move |id| {
      let workspace = workspace.clone();
      async move {
        orchester_workspace::background::run(move || workspace.last_plan_reading(id))
          .await?
          .map(Stored)
          .map_err(|error| error.message())
      }
    })
    .state()
    .get()
    .data
    .and_then(|stored| stored.0);
  match (session, stored) {
    (Some(session), Some(stored)) if stored.0.at > session.0.at => Some(stored),
    (Some(session), _) => Some(session),
    (None, stored) => stored,
  }
}

/// One plan window: its name, when it resets, and the share used over its meter (`5-hour` `WMgdz`).
fn window_row(ctx: &mut Ctx, window: &LimitWindow, reading: &LimitReading, now: Timestamp) -> Element {
  let name = match window.minutes() {
    FIVE_HOURS => t(ctx, "accounts.planLimits.fiveHour"),
    WEEK => t(ctx, "accounts.planLimits.weekly"),
    minutes => ta(
      ctx,
      "accounts.planLimits.window",
      [("hours", (minutes / 60).to_string())],
    ),
  };
  let (used, reached) = share(window, reading, now);
  let resets = window
    .resets()
    .map(|resets| when(ctx, resets, now, ("accounts.time.today", "accounts.time.day")));
  let resets = match (reached, resets) {
    (true, Some(time)) => Some(ta(ctx, "accounts.planLimits.reached", [("time", time)])),
    (false, Some(time)) => Some(ta(ctx, "accounts.planLimits.resets", [("time", time)])),
    (_, None) => None,
  };
  let id = format!("account-window-{}", window.minutes());
  let (tone, color, hint) = if reached {
    (MeterTone::Reached, PaletteColor::Warning, HintTone::Warning)
  } else {
    (MeterTone::Plain, PaletteColor::TextSecondary, HintTone::Muted)
  };
  let gauge = Column::new()
    .id(format!("{id}-gauge"))
    .width(Dimension::Px(METER_WIDTH))
    .spacing(ExtraSpacing::Fine.role())
    .align_items(Alignment::End)
    .describe("reached", reached.to_string())
    .child(
      Text::new(&ta(ctx, "accounts.planLimits.used", [("percent", used.to_string())]))
        .id(format!("{id}-used"))
        .variant(TypographyStyle::Caption)
        .color(color)
        .nowrap(),
    )
    .child(meter(&format!("{id}-meter"), (used, 100), tone));
  let row = SettingRow::new(id, name).control(gauge);
  match resets {
    Some(resets) => row.toned_hint(resets, hint).render(ctx),
    None => row.render(ctx),
  }
}
