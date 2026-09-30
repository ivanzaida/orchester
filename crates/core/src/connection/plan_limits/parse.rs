use serde_json::Value;

use super::{Credits, LimitState, LimitWindow, PlanLimits};
use crate::time::Timestamp;

/// Session snake_case and App Server camelCase use the same window semantics.
pub fn codex_limits(value: &Value) -> Option<PlanLimits> {
  value.as_object()?;
  let mut windows = Vec::new();
  let reached = optional_text(field(value, "rate_limit_reached_type", "rateLimitReachedType"))?;
  for name in ["primary", "secondary"] {
    if let Some(window) = value.get(name).filter(|window| !window.is_null()) {
      let minutes = field(window, "window_minutes", "windowDurationMins")?.as_u64()?;
      let percent = field(window, "used_percent", "usedPercent")?.as_f64()?;
      let resets = match field(window, "resets_at", "resetsAt") {
        None | Some(Value::Null) => None,
        Some(value) => Some(Timestamp::from_unix_seconds(value.as_i64()?)),
      };
      let state = if reached.is_some_and(|kind| {
        kind == name || (kind == "five_hour" && minutes == 300) || (kind == "seven_day" && minutes == 10080)
      }) {
        LimitState::Reached
      } else {
        LimitState::Normal
      };
      windows.push(LimitWindow::new(minutes, percent, resets, state, false)?);
    }
  }
  // No reported window is not a quota snapshot and must not clear the last valid reading.
  if windows.is_empty() {
    return None;
  }
  if reached.is_some()
    && !windows.iter().any(|window| window.state == LimitState::Reached)
    && let Some(window) = windows.iter_mut().max_by_key(|window| window.used)
  {
    window.state = LimitState::Reached;
  }
  let credits = match value.get("credits") {
    None | Some(Value::Null) => None,
    Some(value) => Some(Credits {
      has_credits: field(value, "has_credits", "hasCredits")?.as_bool()?,
      unlimited: value.get("unlimited")?.as_bool()?,
      balance: optional_text(value.get("balance"))?.map(str::to_owned),
    }),
  };
  Some(PlanLimits {
    windows,
    credits,
    plan: optional_text(field(value, "plan_type", "planType"))?.map(str::to_owned),
  })
}

fn optional_text(value: Option<&Value>) -> Option<Option<&str>> {
  match value {
    None | Some(Value::Null) => Some(None),
    Some(value) => value.as_str().map(Some),
  }
}

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

fn field<'a>(value: &'a Value, snake: &str, camel: &str) -> Option<&'a Value> {
  value.get(snake).or_else(|| value.get(camel))
}
