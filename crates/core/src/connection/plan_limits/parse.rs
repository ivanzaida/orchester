use serde_json::Value;

use super::{Credits, LimitState, LimitWindow, PlanLimits};
use crate::time::Timestamp;

/// The bucket of the account's plan in Codex App Server's `RateLimitSnapshot`; other buckets
/// (`limitId`) meter a single model and are not the plan's limits.
const CODEX_PLAN_BUCKET: &str = "codex";

/// The `rateLimits` snapshot of Codex App Server's `account/rateLimits/updated` notification.
/// The update is sparse: a window without its duration cannot be labelled and is left out, and
/// an update with no window is not a reading ([`super::PlanReadings::observe`] merges the rest).
pub fn codex_limits(value: &Value) -> Option<PlanLimits> {
  value.as_object()?;
  if optional_text(value.get("limitId"))?.is_some_and(|bucket| bucket != CODEX_PLAN_BUCKET) {
    return None;
  }
  // Values name why the limit was reached (`rate_limit_reached`, depleted credits), not a window.
  let reached = optional_text(value.get("rateLimitReachedType"))?.is_some();
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
      windows.push(LimitWindow::new(minutes, percent, resets, LimitState::Normal, false)?);
    }
  }
  // No reported window is not a quota snapshot and must not clear the last valid reading.
  if windows.is_empty() {
    return None;
  }
  if reached && let Some(window) = windows.iter_mut().max_by_key(|window| window.used) {
    window.state = LimitState::Reached;
  }
  let credits = match value.get("credits") {
    None | Some(Value::Null) => None,
    Some(value) => Some(Credits {
      has_credits: value.get("hasCredits")?.as_bool()?,
      unlimited: value.get("unlimited")?.as_bool()?,
      balance: optional_text(value.get("balance"))?.map(str::to_owned),
    }),
  };
  Some(PlanLimits {
    windows,
    credits,
    plan: optional_text(value.get("planType"))?.map(str::to_owned),
  })
}

fn optional_text(value: Option<&Value>) -> Option<Option<&str>> {
  match value {
    None | Some(Value::Null) => Some(None),
    Some(value) => value.as_str().map(Some),
  }
}

/// The `rate_limit_info` of Claude Code's `stream-json` `rate_limit_event`.
pub fn claude_limits(value: &Value) -> Option<PlanLimits> {
  let minutes = match value.get("rateLimitType")?.as_str()? {
    "five_hour" => 300,
    "seven_day" => 10080,
    _ => value.get("window_minutes")?.as_u64()?,
  };
  let state = match value.get("status")?.as_str()? {
    "allowed" => LimitState::Normal,
    "allowed_warning" => LimitState::Near,
    "rejected" => LimitState::Reached,
    _ => return None,
  };
  let extra = value.get("isUsingOverage").and_then(Value::as_bool).unwrap_or(false);
  let resets = value
    .get("resetsAt")
    .and_then(Value::as_i64)
    .map(Timestamp::from_unix_seconds);
  let window = LimitWindow::new(
    minutes,
    value.get("utilization")?.as_f64()? * 100.0,
    resets,
    state,
    extra,
  )?;
  Some(PlanLimits {
    windows: vec![window],
    plan: None,
    credits: None,
  })
}
