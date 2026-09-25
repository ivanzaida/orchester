//! What the inbox says about its sync: the topbar sync status (`k5p3Tg`, `Y2Qw9I`, `JspuL`,
//! `SzB9U`, `kbcMV`, `dOFk6`) and the times it names.

use std::{
  sync::Arc,
  time::{Duration, UNIX_EPOCH},
};

use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::{GitHubLogin, GitHubState, REQUIRED_SCOPE},
  pull_request::SyncFailure,
  repository::RepositoryError,
  time::Timestamp,
};

use crate::{
  display,
  messages::Message,
  pull_requests::{Inbox, LinkedInbox},
  screens::github::status::status_label,
  ui::message_text,
};

const MINUTE_MILLIS: i64 = 60_000;
const HOUR_MILLIS: i64 = 60 * MINUTE_MILLIS;
const DAY_MILLIS: i64 = 24 * HOUR_MILLIS;

/// The state of sync the topbar reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncLine {
  NotConnected,
  Syncing,
  Synced(Timestamp),
  /// GitHub was not reached; `last` is the last successful sync.
  Unreachable {
    last: Option<Timestamp>,
  },
  /// GitHub was reached but failed to answer (a server error).
  Troubled {
    last: Option<Timestamp>,
  },
  /// GitHub was reached but gave no list.
  Failed {
    last: Option<Timestamp>,
  },
  RateLimited {
    resets_at: Option<Timestamp>,
  },
  /// `gh` acts as `active`, another account than the project expects.
  Paused {
    active: GitHubLogin,
  },
  /// `gh` cannot be used as the expected account for another reason.
  Account(GitHubState),
}

impl lurq::app::component::DevtoolsInspectable for SyncLine {}

impl SyncLine {
  /// A running sync is reported over the last outcome, except that nothing syncs without a link.
  pub fn of(inbox: Option<&Inbox>, syncing: bool) -> Self {
    match inbox {
      Some(Inbox::Unlinked) => Self::NotConnected,
      Some(Inbox::Linked(inbox)) if !syncing => Self::after(inbox),
      // Neither the stored list nor a sync gave anything: reading them failed.
      None if !syncing => Self::Failed { last: None },
      Some(Inbox::Linked(_)) | None => Self::Syncing,
    }
  }

  fn after(inbox: &LinkedInbox) -> Self {
    Self::after_read(
      inbox.synced.as_ref().map(|synced| synced.synced_at),
      inbox.failure.as_ref(),
    )
  }

  /// After a sync or refresh that failed with `failure`, if it did; `last` is when what is shown
  /// was read.
  pub fn after_read(last: Option<Timestamp>, failure: Option<&SyncFailure>) -> Self {
    match failure {
      None => last.map_or(Self::Syncing, Self::Synced),
      Some(SyncFailure::RateLimited { resets_at }) => Self::RateLimited { resets_at: *resets_at },
      Some(SyncFailure::Account(GitHubState::WrongAccount { active, .. })) => Self::Paused { active: active.clone() },
      Some(SyncFailure::Account(state)) => Self::Account(state.clone()),
      Some(SyncFailure::Repository(RepositoryError::Network { .. })) => Self::Unreachable { last },
      Some(SyncFailure::Repository(RepositoryError::ServerError { .. })) => Self::Troubled { last },
      Some(SyncFailure::Repository(_)) => Self::Failed { last },
    }
  }

  /// The status text at `now`.
  pub fn text(&self, ctx: &Ctx, now: Timestamp) -> Arc<str> {
    let since = |key, never, last: &Option<Timestamp>| match last {
      Some(last) => ctx.t_args(key, [("ago", &*ago(ctx, now, *last))]),
      None => ctx.t(never),
    };
    match self {
      Self::NotConnected => ctx.t("pullRequests.sync.notConnected"),
      Self::Syncing => ctx.t("pullRequests.sync.syncing"),
      Self::Synced(at) => ctx.t_args("pullRequests.sync.synced", [("ago", &*ago(ctx, now, *at))]),
      Self::Unreachable { last } => since(
        "pullRequests.sync.unreachable",
        "pullRequests.sync.neverUnreachable",
        last,
      ),
      Self::Troubled { last } => since("pullRequests.sync.troubled", "pullRequests.sync.neverTroubled", last),
      Self::Failed { last } => since("pullRequests.sync.failed", "pullRequests.sync.neverFailed", last),
      Self::RateLimited { resets_at: Some(at) } => ctx.t_args(
        "pullRequests.sync.rateLimited",
        [("minutes", minutes_until(now, *at).to_string().as_str())],
      ),
      Self::RateLimited { resets_at: None } => ctx.t("pullRequests.sync.rateLimitedUnknown"),
      Self::Paused { active } => ctx.t_args("pullRequests.sync.paused", [("account", active.as_str())]),
      Self::Account(state) => ctx.t_args(status_label(state).1, [("scope", REQUIRED_SCOPE)]),
    }
  }

  /// The exact local time of a rate limit's reset, for the status text's tooltip (`Wn3fn`).
  pub fn reset_hint(&self, ctx: &Ctx) -> Option<Arc<str>> {
    match self {
      Self::RateLimited { resets_at: Some(at) } => {
        Some(ctx.t_args("pullRequests.sync.resetsAt", [("time", local_time(*at).as_str())]))
      }
      _ => None,
    }
  }
}

/// How long ago `then` was, as the sync texts say it ("14 min ago"): lists say "14m ago"
/// (`display::elapsed`), the sync status spells the unit out.
fn ago(ctx: &Ctx, now: Timestamp, then: Timestamp) -> Arc<str> {
  let millis = now.unix_millis() - then.unix_millis();
  let (key, count) = if millis >= DAY_MILLIS {
    ("pullRequests.age.days", millis / DAY_MILLIS)
  } else if millis >= HOUR_MILLIS {
    ("pullRequests.age.hours", millis / HOUR_MILLIS)
  } else if millis >= MINUTE_MILLIS {
    ("pullRequests.age.minutes", millis / MINUTE_MILLIS)
  } else {
    return ctx.t("pullRequests.age.justNow");
  };
  message_text(ctx, &Message::new(key).with("count", count.to_string()))
}

/// Whole minutes until `at`, rounded up and at least one, so a limit never reads "resets in 0 min".
pub fn minutes_until(now: Timestamp, at: Timestamp) -> i64 {
  let millis = at.unix_millis() - now.unix_millis();
  (millis.max(0) + MINUTE_MILLIS - 1).div_euclid(MINUTE_MILLIS).max(1)
}

/// Local wall-clock time of `at`, e.g. `14:05`.
fn local_time(at: Timestamp) -> String {
  // Times shown here are sync and reset times, never before 1970.
  let millis = u64::try_from(at.unix_millis().max(0)).expect("a non-negative i64 fits in u64");
  display::clock_time(UNIX_EPOCH + Duration::from_millis(millis))
}

/// Local time of the last successful sync, which the list on screen is from.
pub fn list_time(inbox: &LinkedInbox) -> Option<String> {
  inbox.synced.as_ref().map(|synced| local_time(synced.synced_at))
}
