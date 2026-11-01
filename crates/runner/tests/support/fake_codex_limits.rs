//! The plan limits a turn of `orchester-fake-codex` reports, and how it fails at them, by its prompt:
//!
//! - "Spend credits past the window": first sends App Server's snapshot as Codex 0.164.0 read it on 2026-11-01: the
//!   five-hour window used up with `rate_limit_reached`, the weekly one at 41 %, and credits it can spend; then works
//!   as usual;
//! - "Fill the window": the same snapshot without credits to spend; then works as usual;
//! - "Fail for another reason": as "Fill the window", then the turn fails for a reason Codex names that is no usage
//!   limit (`httpConnectionFailed`);
//! - "Refuse at the usage limit": as "Spend credits past the window", then Codex refuses the turn at the usage limit
//!   (`usageLimitExceeded`) all the same.

use std::{
  process::ExitCode,
  time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};

use super::{Session, Step, emit, notify};

/// Sends the limits `prompt` asks for; the step when the turn fails at once, `None` when it works on.
pub fn work(session: &Session, prompt: &str) -> Option<Step> {
  let with_credits = ["Spend credits past the window", "Refuse at the usage limit"]
    .iter()
    .any(|words| prompt.contains(words));
  let without_credits = ["Fill the window", "Fail for another reason"]
    .iter()
    .any(|words| prompt.contains(words));
  if !with_credits && !without_credits {
    return None;
  }
  emit(&json!({"method": "account/rateLimits/updated", "params": {"rateLimits": snapshot(with_credits)}}));
  if prompt.contains("Refuse at the usage limit") {
    return Some(fail(
      session,
      "You've hit your usage limit.",
      &json!("usageLimitExceeded"),
    ));
  }
  if prompt.contains("Fail for another reason") {
    return Some(fail(
      session,
      "stream disconnected before completion",
      &json!({"httpConnectionFailed": {"httpStatusCode": 502}}),
    ));
  }
  None
}

/// The plan's bucket with its five-hour window used up, which resets in an hour.
fn snapshot(with_credits: bool) -> Value {
  let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |since| since.as_secs());
  let credits = if with_credits {
    json!({"hasCredits": true, "unlimited": false, "balance": "70412.5"})
  } else {
    json!({"hasCredits": false, "unlimited": false, "balance": "0"})
  };
  json!({
    "limitId": "codex",
    "limitName": null,
    "primary": {"usedPercent": 100, "windowDurationMins": 300, "resetsAt": now + 3_600},
    "secondary": {"usedPercent": 41, "windowDurationMins": 10_080, "resetsAt": now + 86_400},
    "credits": credits,
    "spendControlReached": false,
    "planType": "pro",
    "rateLimitReachedType": "rate_limit_reached",
    "individualLimit": null
  })
}

/// Ends the turn failed with `message`, for the reason `info` names.
fn fail(session: &Session, message: &str, info: &Value) -> Step {
  let error = json!({"message": message, "codexErrorInfo": info, "additionalDetails": null});
  notify(session, "error", json!({"error": error, "willRetry": false}));
  notify(
    session,
    "turn/completed",
    json!({"turn": {"id": session.turn, "status": "failed", "error": error}}),
  );
  Step::Exit(ExitCode::SUCCESS)
}
