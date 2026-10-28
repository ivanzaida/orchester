//! *Plan limits* (`Plan limits` `Q6pBK` in 01 `LXhun`): only the accounts near or at a limit
//! (`orchester_workspace::overview::limits`), each its CLI's mark, *Claude Code · Work* and the share of its window
//! (`warning` when near, *Reached · 5-hour* in `danger`), a 4 high meter filled in that tone, and when it resets with
//! how many runs at work run on it. A reading whose window reset since shows greyed, *unknown since reset*, with what
//! it last read (05 `JzNRO`). *Accounts ↗* opens Settings › Accounts. The readings are the session's and those kept,
//! drawn at once from memory.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  account::{AccountCli, AgentAccount},
  connection::plan_limits::LimitWindow,
  time::Timestamp,
};
use orchester_workspace::{
  display::{self, WEEKDAY_KEYS},
  messages::Message,
  overview::{
    at_work::{AtWorkLine, runs_on},
    limits::{LimitLook, NearLimit},
  },
};

use super::section::{head, link, section};
use crate::{
  navigation::Navigation,
  routes::{AccountsView, Route},
  services::accounts::{Accounts, account_name},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OverviewText},
  ui::{agent_mark::AgentMark, message::message_text, plan_window::share, pressable::Press},
};

/// A five-hour window and a weekly one, in minutes; the meter's height (`J7gvL6`).
const FIVE_HOURS: u64 = 300;
const WEEK: u64 = 10_080;
const METER_HEIGHT: f32 = 4.0;

/// The section, `None` when no account is near a limit.
pub(super) fn limits(
  ctx: &mut Ctx,
  (near, accounts): (&[NearLimit], &Accounts),
  at_work: &[AtWorkLine],
  now: Timestamp,
) -> Option<Column> {
  let shown: Vec<Element> = near
    .iter()
    .filter_map(|limit| Some(item(ctx, (limit, accounts.of(limit.account)?), at_work, now)))
    .collect();
  if shown.is_empty() {
    return None;
  }
  let open = Navigation::of(ctx).to(Route::Accounts(AccountsView::default()));
  let accounts_link = link(
    "overview-limits-accounts",
    &ctx.t("overview.limits.accounts"),
    Press::Open(open),
  );
  Some(
    section("overview-limits")
      .child(head(
        "overview-limits",
        &ctx.t("overview.limits.title"),
        None,
        Some(accounts_link),
      ))
      .with_children(shown),
  )
}

fn item(ctx: &Ctx, (limit, account): (&NearLimit, &AgentAccount), at_work: &[AtWorkLine], now: Timestamp) -> Element {
  let id = format!("overview-limit-{}", account.id());
  let at = if limit.look == LimitLook::ResetSince {
    limit.reading.at
  } else {
    now
  };
  let (used, _) = limit
    .window
    .as_ref()
    .map_or((100, true), |window| share(window, &limit.reading, at));
  let window = limit.window.as_ref().map(|window| window_name(ctx, window));
  let (value, tone) = match (limit.look, &window) {
    (LimitLook::Near, Some(window)) => (
      ctx.t_args(
        "overview.limits.share",
        [("used", used.to_string()), ("window", window.clone())],
      ),
      PaletteColor::Warning,
    ),
    (LimitLook::Reached, Some(window)) => (
      ctx.t_args("overview.limits.reached", [("window", window.clone())]),
      PaletteColor::Danger,
    ),
    (LimitLook::ResetSince, _) => (ctx.t("overview.limits.unknown"), PaletteColor::TextMuted),
    (_, None) => (ctx.t("overview.limits.reachedOnly"), PaletteColor::Danger),
  };
  let name = ctx.t_args(
    "overview.limits.name",
    [
      ("cli", cli_name(ctx, account.cli())),
      ("account", account_name(ctx, account)),
    ],
  );
  let line = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(AgentMark::of_cli(account.cli()).render(ctx, IconSize::Sm))
    .child(
      Text::new(&name)
        .variant(OverviewText::Line.role())
        .color(if limit.look == LimitLook::ResetSince {
          PaletteColor::TextMuted
        } else {
          PaletteColor::TextPrimary
        })
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&value)
        .id(format!("{id}-value"))
        .variant(TypographyStyle::Description)
        .color(tone.clone())
        .nowrap(),
    );
  let note = note(
    ctx,
    limit,
    (used, window.as_deref()),
    (runs_on(at_work, account.id()), now),
  );
  Column::new()
    .id(id.clone())
    .describe("look", look_name(limit.look))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Snug.role())
    .child(line)
    .child(meter(&format!("{id}-meter"), used, tone))
    .child(
      Text::new(&note)
        .id(format!("{id}-note"))
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    )
    .into()
}

/// When it resets and how many runs at work run on it, or, once its window reset since, what it last read.
fn note(ctx: &Ctx, limit: &NearLimit, (used, window): (u64, Option<&str>), (runs, now): (usize, Timestamp)) -> String {
  let resets = limit.window.as_ref().and_then(LimitWindow::resets);
  if limit.look == LimitLook::ResetSince {
    return ctx
      .t_args(
        "overview.limits.lastRead",
        [
          ("used", used.to_string()),
          ("window", window.unwrap_or_default().to_owned()),
          ("time", resets.map(|at| reset_time(ctx, at, now)).unwrap_or_default()),
        ],
      )
      .to_string();
  }
  let count = runs.to_string();
  let words = match (resets, runs) {
    (Some(at), 0) => ctx.t_args("overview.limits.resets", [("time", reset_time(ctx, at, now))]),
    (Some(at), 1) => ctx.t_args(
      "overview.limits.resetsRuns.one",
      [("time", reset_time(ctx, at, now)), ("count", count)],
    ),
    (Some(at), _) => ctx.t_args(
      "overview.limits.resetsRuns.other",
      [("time", reset_time(ctx, at, now)), ("count", count)],
    ),
    (None, 0) => return String::new(),
    (None, 1) => ctx.t_args("overview.limits.runs.one", [("count", count)]),
    (None, _) => ctx.t_args("overview.limits.runs.other", [("count", count)]),
  };
  words.to_string()
}

/// *17:40* today, *Thu 09:00* on another day.
fn reset_time(ctx: &Ctx, at: Timestamp, now: Timestamp) -> String {
  let time = display::clock(at);
  if display::calendar_date(at) == display::calendar_date(now) {
    return time;
  }
  let weekday = message_text(ctx, &Message::new(WEEKDAY_KEYS[display::weekday(at)]));
  ctx
    .t_args(
      "overview.limits.day",
      [("weekday", weekday.to_string()), ("time", time)],
    )
    .to_string()
}

fn window_name(ctx: &Ctx, window: &LimitWindow) -> String {
  match window.minutes() {
    FIVE_HOURS => ctx.t("overview.limits.fiveHour").to_string(),
    WEEK => ctx.t("overview.limits.weekly").to_string(),
    minutes => ctx
      .t_args("overview.limits.window", [("hours", (minutes / 60).to_string())])
      .to_string(),
  }
}

fn cli_name(ctx: &Ctx, cli: AccountCli) -> String {
  ctx
    .t(match cli {
      AccountCli::ClaudeCode => "overview.limits.claude",
      AccountCli::Codex => "overview.limits.codex",
    })
    .to_string()
}

/// A 4 high `border` track with `used` percent of it filled in `tone`.
fn meter(id: &str, used: u64, tone: PaletteColor) -> Row {
  // A whole percent of a few hundred pixels: f32 is precise enough.
  let percent = used.min(100) as f32;
  Row::new()
    .id(id.to_owned())
    .describe("share", used.to_string())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(METER_HEIGHT))
    .background(PaletteColor::Border)
    .rounded(ExtraRadius::Meter.role())
    .child(
      Column::new()
        .width(Dimension::Pct(percent))
        .height(Dimension::Pct(100.0))
        .background(tone)
        .rounded(ExtraRadius::Meter.role()),
    )
}

const fn look_name(look: LimitLook) -> &'static str {
  match look {
    LimitLook::Near => "near",
    LimitLook::Reached => "reached",
    LimitLook::ResetSince => "reset-since",
  }
}
