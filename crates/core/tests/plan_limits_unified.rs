//! Claude Code's `unifiedWindows`: every plan window's use in each `rate_limit_event`, and two accounts that report
//! different windows keeping their own. The events are shaped as Claude Code 2.1.301 sent them on 2026-10-30.

use orchester_core::{
  account::AccountCli,
  connection::{
    AgentProvider,
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings, claude_limits},
  },
  id::AgentAccountId,
  time::Timestamp,
};
use serde_json::{Value, json};

const FIVE_HOUR_RESET: i64 = 1_793_394_600;
const DEFAULT_WEEK_RESET: i64 = 1_793_746_800;
const SECOND_WEEK_RESET: i64 = 1_793_563_200;

fn now() -> Timestamp {
  Timestamp::from_unix_seconds(1_793_379_600)
}

/// A turn on the Default account: the event names the five-hour window without its use.
fn default_event() -> Value {
  json!({
    "status": "allowed", "resetsAt": FIVE_HOUR_RESET, "rateLimitType": "five_hour", "isUsingOverage": false,
    "unifiedWindows": {
      "five_hour": {"utilization": 0.1, "resetsAt": FIVE_HOUR_RESET},
      "seven_day": {"utilization": 0.66, "resetsAt": DEFAULT_WEEK_RESET},
    },
  })
}

/// A turn on another account, past its weekly warning threshold.
fn second_event() -> Value {
  json!({
    "status": "allowed_warning", "resetsAt": SECOND_WEEK_RESET, "rateLimitType": "seven_day", "utilization": 0.84,
    "isUsingOverage": false, "surpassedThreshold": 0.75,
    "unifiedWindows": {
      "five_hour": {"utilization": 0.09, "resetsAt": FIVE_HOUR_RESET},
      "seven_day": {"utilization": 0.84, "resetsAt": SECOND_WEEK_RESET},
      "seven_day_opus": {"utilization": 0.5, "resetsAt": SECOND_WEEK_RESET},
    },
  })
}

fn window(limits: &PlanLimits, minutes: u64) -> &LimitWindow {
  limits
    .windows
    .iter()
    .find(|window| window.minutes() == minutes)
    .unwrap_or_else(|| panic!("a {minutes}-minute window in {limits:?}"))
}

fn reading(account: AgentAccountId, limits: PlanLimits) -> LimitReading {
  LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    account: Some(account),
    at: now(),
    limits,
  }
}

#[test]
fn plan_limits_unified_windows_fill_the_named_window_and_add_the_others() {
  let limits = claude_limits(&default_event()).expect("limits");
  assert_eq!(limits.windows.len(), 2);
  let five = window(&limits, 300);
  assert_eq!(
    five.percent(),
    Some(10.0),
    "the named window's use comes from its unified entry"
  );
  assert_eq!(five.resets(), Some(Timestamp::from_unix_seconds(FIVE_HOUR_RESET)));
  let weekly = window(&limits, 10080);
  assert_eq!(weekly.percent(), Some(66.0));
  assert_eq!(weekly.resets(), Some(Timestamp::from_unix_seconds(DEFAULT_WEEK_RESET)));
  assert_eq!(weekly.state(now()), LimitState::Normal);

  let limits = claude_limits(&second_event()).expect("limits");
  assert_eq!(limits.windows.len(), 2, "a single model's bucket is not the plan's");
  assert_eq!(window(&limits, 10080).percent(), Some(84.0));
  assert_eq!(window(&limits, 10080).state(now()), LimitState::Near);
  assert_eq!(window(&limits, 300).percent(), Some(9.0));
}

#[test]
fn plan_limits_unified_windows_without_a_named_window_or_malformed() {
  let unnamed = json!({"status": "allowed", "unifiedWindows": {"seven_day": {"utilization": 1.0}}});
  let limits = claude_limits(&unnamed).expect("the unified window alone");
  assert_eq!(window(&limits, 10080).state(now()), LimitState::Reached);
  assert!(limits.blocked.is_none());
  let malformed = json!({"status": "allowed", "rateLimitType": "five_hour",
    "unifiedWindows": {"seven_day": {"utilization": "high"}}});
  assert!(
    claude_limits(&malformed).is_none(),
    "an event this version cannot read is dropped"
  );
  let empty = json!({"status": "allowed", "unifiedWindows": {}});
  assert!(claude_limits(&empty).is_none(), "nothing to show");
}

#[test]
fn plan_limits_two_claude_accounts_keep_their_own_readings() {
  let default = AccountCli::ClaudeCode.default_account();
  let second = AgentAccountId::generate();
  let mut readings = PlanReadings::default();
  assert!(readings.observe(reading(default, claude_limits(&default_event()).expect("Default"))));
  assert!(readings.observe(reading(second, claude_limits(&second_event()).expect("second"))));
  let later = Timestamp::from_unix_millis(now().unix_millis() + 60_000);
  let mut newer = reading(second, claude_limits(&second_event()).expect("second"));
  newer.at = later;
  readings.observe(newer);

  let own = readings.of_account(default).expect("Default's reading");
  assert_eq!(own.at, now(), "another account's newer reading is not Default's");
  assert_eq!(window(&own.limits, 10080).percent(), Some(66.0));
  assert_eq!(
    window(&own.limits, 10080).resets(),
    Some(Timestamp::from_unix_seconds(DEFAULT_WEEK_RESET))
  );
  let other = readings.of_account(second).expect("the second account's reading");
  assert_eq!(other.at, later);
  assert_eq!(window(&other.limits, 10080).percent(), Some(84.0));
  assert_eq!(
    window(&other.limits, 10080).resets(),
    Some(Timestamp::from_unix_seconds(SECOND_WEEK_RESET))
  );
}
