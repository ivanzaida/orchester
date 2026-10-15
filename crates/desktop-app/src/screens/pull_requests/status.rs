//! The sync status under Refresh (`Sync status` `L8YDu5`, its states in `Pull requests · sync states`
//! `uuaob`): a 12 icon and the words in 11.5, 6 apart. It reports only what happened: the time the list
//! shown was read, the reset GitHub gave, the account `gh` uses. A rate limit's exact local reset time is
//! in its tooltip (`P8F96`).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  layout::Alignment,
  node::Element,
};
use orchester_core::{
  connection::{GitHubState, REQUIRED_SCOPE},
  time::Timestamp,
};
use orchester_workspace::{display, pull_requests::status::SyncStatus};

use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    tooltip::Tooltip,
  },
};

const MINUTE_MILLIS: i64 = 60_000;

/// What the status line says, and how.
pub(super) enum Shown {
  /// Nothing is linked: GitHub is not connected (`ggZ64` in 08.D).
  NotConnected,
  /// A sync or *Use this account* runs.
  Syncing,
  After(SyncStatus),
}

/// The status line, its element id `pull-requests-sync-status`.
pub(super) fn status_line(ctx: &mut Ctx, shown: &Shown, now: Timestamp) -> Element {
  let (glyph, mark, color, words) = parts(ctx, shown, now);
  let line = Row::new()
    .id("pull-requests-sync-status")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("a11y_name", words.clone())
    .child(icon(glyph, IconSize::Sm, mark))
    .child(
      Text::new(&words)
        .id("pull-requests-sync-status-text")
        .variant(ExtraTypography::Note.role())
        .color(color)
        .nowrap(),
    );
  match shown {
    Shown::After(SyncStatus::RateLimited { resets_at: Some(at) }) => {
      let hint = ctx.t_args("pull_requests.sync.resetsAt", [("time", display::clock(*at))]);
      Tooltip::new("pull-requests-sync-status", hint).end().wrap(ctx, line)
    }
    _ => line.into(),
  }
}

/// The icon, its color, the words' color and the words.
pub(super) fn parts(ctx: &Ctx, shown: &Shown, now: Timestamp) -> (Icon, PaletteColor, PaletteColor, Arc<str>) {
  use PaletteColor::{Danger, TextMuted, TextSecondary, Warning};
  let status = match shown {
    Shown::NotConnected => {
      return (
        Icon::Unplug,
        TextMuted,
        TextMuted,
        ctx.t("pull_requests.sync.notConnected"),
      );
    }
    Shown::Syncing => {
      return (
        Icon::LoaderCircle,
        TextSecondary,
        TextSecondary,
        ctx.t("pull_requests.sync.syncing"),
      );
    }
    Shown::After(status) => status,
  };
  let since = |key: &str, never: &str, last: &Option<Timestamp>| match last {
    Some(last) => ctx.t_args(key, [("time", display::clock(*last))]),
    None => ctx.t(never),
  };
  match status {
    SyncStatus::NotSynced => (
      Icon::CircleDashed,
      TextMuted,
      TextMuted,
      ctx.t("pull_requests.sync.notSynced"),
    ),
    SyncStatus::Synced(at) => (Icon::CircleCheck, TextMuted, TextMuted, synced(ctx, now, *at)),
    SyncStatus::Unreachable { last } => (
      Icon::CloudOff,
      Danger,
      TextSecondary,
      since(
        "pull_requests.sync.unreachable",
        "pull_requests.sync.neverUnreachable",
        last,
      ),
    ),
    SyncStatus::Troubled { last } => (
      Icon::ServerCrash,
      Danger,
      TextSecondary,
      since("pull_requests.sync.troubled", "pull_requests.sync.neverTroubled", last),
    ),
    SyncStatus::Failed { last } => (
      Icon::CircleAlert,
      Danger,
      TextSecondary,
      since("pull_requests.sync.failed", "pull_requests.sync.neverFailed", last),
    ),
    SyncStatus::RateLimited { resets_at } => (
      Icon::Timer,
      TextSecondary,
      TextSecondary,
      rate_limited(ctx, now, *resets_at),
    ),
    SyncStatus::Paused { active } => (
      Icon::CirclePause,
      Warning,
      Warning,
      ctx.t_args("pull_requests.sync.paused", [("account", active.as_str())]),
    ),
    SyncStatus::Account(state) => (Icon::CircleAlert, Warning, Warning, account(ctx, state)),
  }
}

/// *Synced just now* within a minute of `at`, else how long ago.
fn synced(ctx: &Ctx, now: Timestamp, at: Timestamp) -> Arc<str> {
  ctx.t_args(
    "pull_requests.sync.synced",
    [("ago", message_text(ctx, &display::ago(now, at)))],
  )
}

fn rate_limited(ctx: &Ctx, now: Timestamp, resets_at: Option<Timestamp>) -> Arc<str> {
  match resets_at {
    Some(at) => ctx.t_args(
      "pull_requests.sync.rateLimited",
      [("minutes", minutes_until(now, at).to_string())],
    ),
    None => ctx.t("pull_requests.sync.rateLimitedUnknown"),
  }
}

/// Whole minutes until `at`, rounded up and at least one, so a limit never reads "resets in 0 min".
pub(super) fn minutes_until(now: Timestamp, at: Timestamp) -> i64 {
  let millis = at.unix_millis() - now.unix_millis();
  (millis.max(0) + MINUTE_MILLIS - 1).div_euclid(MINUTE_MILLIS).max(1)
}

/// Why `gh` cannot act as the expected account, in the GitHub connection's words.
pub(super) fn account(ctx: &Ctx, state: &GitHubState) -> Arc<str> {
  let key = match state {
    GitHubState::NotChecked => "pull_requests.account.notChecked",
    GitHubState::SignedIn { .. } => "pull_requests.account.signedIn",
    GitHubState::NotSignedIn => "pull_requests.account.notSignedIn",
    GitHubState::SignInUnknown => "pull_requests.account.signInUnknown",
    GitHubState::WrongAccount { .. } => "pull_requests.account.wrongAccount",
    GitHubState::MissingScope { .. } => "pull_requests.account.missingScope",
    GitHubState::ProgramMissing => "pull_requests.account.programMissing",
    GitHubState::Unrecognized => "pull_requests.account.unrecognized",
    GitHubState::CouldNotRun => "pull_requests.account.couldNotRun",
  };
  ctx.t_args(key, [("scope", REQUIRED_SCOPE)])
}
