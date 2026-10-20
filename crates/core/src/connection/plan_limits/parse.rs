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
pub fn claude_limits(value: &Value) -> Option<PlanLimits> {
  let state = match value.get("status")?.as_str()? {
    "allowed" => LimitState::Normal,
    "allowed_warning" => LimitState::Near,
    "rejected" => LimitState::Reached,
    _ => return None,
  };
  let minutes = match value.get("rateLimitType") {
    None | Some(Value::Null) => None,
    Some(kind) => Some(match kind.as_str()? {
      "five_hour" => 300,
      "seven_day" => 10080,
      _ => return None,
    }),
  };
  let Some(minutes) = minutes else {
    // Without its window the event still says whether the CLI refuses new turns.
    return (state == LimitState::Reached).then_some(PlanLimits {
      windows: Vec::new(),
      plan: None,
      credits: None,
      blocked: Some(Blocked {
        reason: LimitBlock::Window,
        window: None,
      }),
    });
  };
  let percent = match value.get("utilization") {
    None | Some(Value::Null) => None,
    Some(value) => Some(value.as_f64()? * 100.0),
  };
  let resets = match value.get("resetsAt") {
    None | Some(Value::Null) => None,
    Some(value) => Some(Timestamp::from_unix_seconds(value.as_i64()?)),
  };
  let extra = value.get("isUsingOverage").and_then(Value::as_bool).unwrap_or(false);
  // A utilization out of range (negative or not a number) is not a reading; the event is dropped
  // like any other this version cannot read, and the last valid window stays.
  let window = LimitWindow::new(minutes, percent, resets, state, extra).ok()?;
  Some(PlanLimits {
    windows: vec![window],
    plan: None,
    credits: None,
    blocked: None,
  })
}
