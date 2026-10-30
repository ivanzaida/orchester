use serde_json::Value;

use super::{Blocked, Credits, LimitBlock, LimitState, LimitWindow, PlanLimits};
use crate::time::Timestamp;

/// The bucket of the account's plan in Codex App Server's `RateLimitSnapshot`; other buckets
/// (`limitId`) meter a single model and are not the plan's limits.
const CODEX_PLAN_BUCKET: &str = "codex";

/// The `rateLimits` snapshot of Codex App Server's `account/rateLimits/updated` notification.
/// The update is sparse: a window without its duration cannot be labelled and is left out, and
/// what it leaves out keeps its last value ([`super::PlanReadings::observe`]). `None` for a
/// malformed snapshot, another bucket, or one that reports nothing.
pub fn codex_limits(value: &Value) -> Option<PlanLimits> {
  value.as_object()?;
  if optional_text(value.get("limitId"))?.is_some_and(|bucket| bucket != CODEX_PLAN_BUCKET) {
    return None;
  }
  let reason = optional_text(value.get("rateLimitReachedType"))?.map(codex_block);
  let mut windows = Vec::new();
  for name in ["primary", "secondary"] {
    if let Some(window) = value.get(name).filter(|window| !window.is_null()) {
      let percent = window.get("usedPercent")?.as_f64()?;
      let resets = match window.get("resetsAt") {
        None | Some(Value::Null) => None,
        Some(value) => Some(Timestamp::from_unix_seconds(value.as_i64()?)),
      };
      let minutes = match window.get("windowDurationMins") {
        None | Some(Value::Null) => continue,
        Some(value) => value.as_u64()?,
      };
      // A window App Server reports out of range (negative or not a number) makes the whole
      // snapshot untrustworthy; it is not a reading, and the last valid one stays.
      windows.push(LimitWindow::new(minutes, Some(percent), resets, LimitState::Normal, false).ok()?);
    }
  }
  // The reason names no window: a window block applies to the fullest window of this update, and
  // to none when it reports no window.
  let blocked = reason.map(|reason| Blocked {
    reason,
    window: (reason == LimitBlock::Window)
      .then(|| {
        windows
          .iter()
          .max_by_key(|window| window.used)
          .map(LimitWindow::minutes)
      })
      .flatten(),
  });
  let credits = match value.get("credits") {
    None | Some(Value::Null) => None,
    Some(value) => Some(Credits {
      has_credits: value.get("hasCredits")?.as_bool()?,
      unlimited: value.get("unlimited")?.as_bool()?,
      balance: optional_text(value.get("balance"))?.map(str::to_owned),
    }),
  };
  let plan = optional_text(value.get("planType"))?.map(str::to_owned);
  if windows.is_empty() && blocked.is_none() && credits.is_none() && plan.is_none() {
    return None;
  }
  Some(PlanLimits {
    windows,
    plan,
    credits,
    blocked,
  })
}

/// The result of Codex App Server's `account/rateLimits/read` (`GetAccountRateLimitsResponse`), read as
/// [`codex_limits`] reads an update: the plan's bucket of `rateLimitsByLimitId` when App Server names buckets,
/// otherwise its single-bucket `rateLimits`. `None` when neither holds a snapshot of the plan.
pub fn codex_read_limits(result: &Value) -> Option<PlanLimits> {
  let by_bucket = result
    .get("rateLimitsByLimitId")
    .and_then(|buckets| buckets.get(CODEX_PLAN_BUCKET))
    .filter(|snapshot| !snapshot.is_null());
  by_bucket.or_else(|| result.get("rateLimits")).and_then(codex_limits)
}

/// App Server's `RateLimitReachedType` names why turns are refused, not a window.
fn codex_block(reason: &str) -> LimitBlock {
  match reason {
    "rate_limit_reached" => LimitBlock::Window,
    "workspace_owner_credits_depleted" | "workspace_member_credits_depleted" => LimitBlock::Credits,
    "workspace_owner_usage_limit_reached" | "workspace_member_usage_limit_reached" => LimitBlock::Workspace,
    _ => LimitBlock::Other,
  }
}

fn optional_text(value: Option<&Value>) -> Option<Option<&str>> {
  match value {
    None | Some(Value::Null) => Some(None),
    Some(value) => value.as_str().map(Some),
  }
}

/// The `rate_limit_info` of Claude Code's `stream-json` `rate_limit_event`. Claude Code leaves
/// out `utilization`, `resetsAt` and `rateLimitType` when it does not know them. Windows of one
/// model (`seven_day_opus`, `seven_day_sonnet`, …) or of another kind of use are not the plan's
/// and give `None`; so does an event with nothing to show.
///
/// The event names one window, often without its use; `unifiedWindows` (Claude Code 2.1.301, read
/// 2026-10-30) gives the use and reset of every plan window as of the same response. Both are
/// taken: what the named window leaves out comes from its unified entry, and the others are added
/// with only their use to tell their state.
pub fn claude_limits(value: &Value) -> Option<PlanLimits> {
  let state = match value.get("status")?.as_str()? {
    "allowed" => LimitState::Normal,
    "allowed_warning" => LimitState::Near,
    "rejected" => LimitState::Reached,
    _ => return None,
  };
  let minutes = match value.get("rateLimitType") {
    None | Some(Value::Null) => None,
    Some(kind) => Some(claude_window_minutes(kind.as_str()?)?),
  };
  let unified = claude_unified_windows(value.get("unifiedWindows"))?;
  let Some(minutes) = minutes else {
    return claude_unnamed(state, &unified);
  };
  let reported = unified.iter().find(|window| window.minutes == minutes);
  let percent = match optional_number(value.get("utilization"))? {
    Some(utilization) => Some(utilization * 100.0),
    None => reported.and_then(|window| window.percent),
  };
  let resets = match value.get("resetsAt") {
    None | Some(Value::Null) => reported.and_then(|window| window.resets),
    Some(value) => Some(Timestamp::from_unix_seconds(value.as_i64()?)),
  };
  let extra = value.get("isUsingOverage").and_then(Value::as_bool).unwrap_or(false);
  // A utilization out of range (negative or not a number) is not a reading; the event is dropped
  // like any other this version cannot read, and the last valid window stays.
  let mut windows = vec![LimitWindow::new(minutes, percent, resets, state, extra).ok()?];
  for other in unified.iter().filter(|window| window.minutes != minutes) {
    windows.push(other.window()?);
  }
  Some(PlanLimits {
    windows,
    plan: None,
    credits: None,
    blocked: None,
  })
}

/// An event that names no window: the unified windows it carries, and whether the CLI refuses new turns.
fn claude_unnamed(state: LimitState, unified: &[UnifiedWindow]) -> Option<PlanLimits> {
  let refused = state == LimitState::Reached;
  if unified.is_empty() && !refused {
    return None;
  }
  let windows = unified.iter().map(UnifiedWindow::window).collect::<Option<Vec<_>>>()?;
  Some(PlanLimits {
    windows,
    plan: None,
    credits: None,
    blocked: refused.then_some(Blocked {
      reason: LimitBlock::Window,
      window: None,
    }),
  })
}

/// The plan's windows by Claude Code's name for them; `None` for any other bucket.
fn claude_window_minutes(kind: &str) -> Option<u64> {
  match kind {
    "five_hour" => Some(300),
    "seven_day" => Some(10080),
    _ => None,
  }
}

/// One plan window of `unifiedWindows`, as reported.
struct UnifiedWindow {
  minutes: u64,
  percent: Option<f64>,
  resets: Option<Timestamp>,
}

impl UnifiedWindow {
  /// The window on its own: no status names it, so its use alone tells its state.
  fn window(&self) -> Option<LimitWindow> {
    LimitWindow::new(self.minutes, self.percent, self.resets, LimitState::Normal, false).ok()
  }
}

/// The plan windows of `unifiedWindows`: empty when it is absent; buckets of one model are passed over; `None` when
/// it is malformed, so the event is dropped as one this version cannot read.
fn claude_unified_windows(value: Option<&Value>) -> Option<Vec<UnifiedWindow>> {
  let entries = match value {
    None | Some(Value::Null) => return Some(Vec::new()),
    Some(value) => value.as_object()?,
  };
  let mut windows = Vec::new();
  for (kind, entry) in entries {
    let Some(minutes) = claude_window_minutes(kind) else {
      continue;
    };
    entry.as_object()?;
    let resets = match entry.get("resetsAt") {
      None | Some(Value::Null) => None,
      Some(value) => Some(Timestamp::from_unix_seconds(value.as_i64()?)),
    };
    windows.push(UnifiedWindow {
      minutes,
      percent: optional_number(entry.get("utilization"))?.map(|utilization| utilization * 100.0),
      resets,
    });
  }
  Some(windows)
}

/// A number that may be left out: `Some(None)` when absent, `None` when it is not a number.
fn optional_number(value: Option<&Value>) -> Option<Option<f64>> {
  match value {
    None | Some(Value::Null) => Some(None),
    Some(value) => value.as_f64().map(Some),
  }
}
