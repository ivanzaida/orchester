//! Whether an account's CLI refuses new turns, apart from how full its included windows are: Codex credits carry work
//! past a used-up window (ORC-592). Run with `cargo test -p orchester-core --test plan_limit_credits
//! plan_limit_credits`.

use orchester_core::{
  connection::{
    AgentProvider,
    app_server::{AppServerStream, CodexFailure, Update},
    plan_limits::{LimitBlock, LimitReading, LimitState, PlanLimits, codex_limits, codex_read_limits},
  },
  time::Timestamp,
};
use serde_json::{Value, json};

fn now() -> Timestamp {
  Timestamp::from_unix_seconds(1_793_500_000)
}

fn at(seconds: i64) -> Timestamp {
  Timestamp::from_unix_seconds(now().unix_millis() / 1000 + seconds)
}

fn reading(limits: PlanLimits) -> LimitReading {
  LimitReading {
    provider: AgentProvider::CodexCli,
    account: None,
    at: now(),
    limits,
  }
}

/// The plan's bucket as Codex 0.164.0 read it on 2026-11-01: the five-hour window used up with `rate_limit_reached`,
/// the weekly one at 41 %, and `credits` beside them.
fn snapshot(credits: &Value) -> Value {
  json!({
    "limitId": "codex",
    "limitName": null,
    "primary": {"usedPercent": 100, "windowDurationMins": 300, "resetsAt": now().unix_millis() / 1000 + 3_600},
    "secondary": {"usedPercent": 41, "windowDurationMins": 10_080, "resetsAt": now().unix_millis() / 1000 + 86_400},
    "credits": credits,
    "spendControlReached": false,
    "planType": "pro",
    "rateLimitReachedType": "rate_limit_reached",
    "individualLimit": null
  })
}

fn usable() -> Value {
  json!({"hasCredits": true, "unlimited": false, "balance": "70412.5"})
}

#[test]
fn plan_limit_credits_the_observed_mixed_read_starts_new_turns_and_still_shows_the_full_window() {
  let read = json!({"rateLimits": snapshot(&usable()), "rateLimitsByLimitId": {"codex": snapshot(&usable())}});
  let reading = reading(codex_read_limits(&read).expect("the plan's bucket"));
  let credits = reading.limits.credits.as_ref().expect("credits");
  assert!(credits.usable(), "{credits:?}");
  assert_eq!(credits.balance.as_deref(), Some("70412.5"));
  assert_eq!(
    reading.limits.blocked.map(|blocked| (blocked.reason, blocked.window)),
    Some((LimitBlock::Window, Some(300))),
    "rate_limit_reached names the full five-hour window"
  );
  assert_eq!(
    reading.state(now()),
    LimitState::Reached,
    "the included window is used up"
  );
  assert_eq!(reading.resets_at(now()), Some(at(3_600)));
  assert!(!reading.refuses_turns(now()), "credits carry new work past it");
}

#[test]
fn plan_limit_credits_an_older_full_window_without_a_block_starts_on_credits() {
  let mut limits = snapshot(&usable());
  limits["rateLimitReachedType"] = Value::Null;
  let reading = reading(codex_limits(&limits).expect("a snapshot"));
  assert_eq!(reading.limits.blocked, None);
  assert_eq!(reading.state(now()), LimitState::Reached);
  assert!(!reading.refuses_turns(now()));
  let unlimited = snapshot(&json!({"hasCredits": false, "unlimited": true, "balance": null}));
  assert!(!reading_of(&unlimited).refuses_turns(now()), "unlimited credits");
}

fn reading_of(limits: &Value) -> LimitReading {
  reading(codex_limits(limits).expect("a snapshot"))
}

#[test]
fn plan_limit_credits_no_spendable_credits_keep_the_full_window_refusing() {
  for credits in [
    json!({"hasCredits": false, "unlimited": false, "balance": "0"}),
    json!({"hasCredits": true, "unlimited": false, "balance": "0"}),
    json!({"hasCredits": true, "unlimited": false, "balance": "-3.5"}),
    json!({"hasCredits": true, "unlimited": false, "balance": "70412.5", "spendControlReached": true}),
    Value::Null,
  ] {
    let reading = reading_of(&snapshot(&credits));
    assert!(reading.refuses_turns(now()), "{credits}");
    assert_eq!(reading.resets_at(now()), Some(at(3_600)), "{credits}");
  }
  let mut controlled = snapshot(&usable());
  controlled["spendControlReached"] = json!(true);
  let reading = reading_of(&controlled);
  assert!(
    reading
      .limits
      .credits
      .as_ref()
      .is_some_and(|credits| credits.spend_control_reached)
  );
  assert!(reading.refuses_turns(now()), "a spend control on the snapshot");
  let unread = json!({"hasCredits": true, "unlimited": false, "balance": null});
  assert!(
    !reading_of(&snapshot(&unread)).refuses_turns(now()),
    "an unreported balance leaves hasCredits to tell"
  );
  let mut malformed = snapshot(&usable());
  malformed["spendControlReached"] = json!("no");
  assert!(
    codex_limits(&malformed).is_none(),
    "a snapshot this version cannot read"
  );
}

#[test]
fn plan_limit_credits_never_lift_a_block_that_is_not_a_window() {
  for reason in [
    "workspace_member_credits_depleted",
    "workspace_owner_credits_depleted",
    "workspace_owner_usage_limit_reached",
    "workspace_member_usage_limit_reached",
    "something_new",
  ] {
    let mut limits = snapshot(&usable());
    limits["rateLimitReachedType"] = json!(reason);
    let reading = reading_of(&limits);
    assert!(reading.refuses_turns(now()), "{reason}");
    assert_eq!(reading.resets_at(now()), None, "{reason} lifts at no reset");
  }
}

#[test]
fn plan_limit_credits_windows_below_their_limit_or_reset_refuse_nothing() {
  let normal = json!({"primary": {"usedPercent": 40, "windowDurationMins": 300}, "credits": null});
  assert!(!reading_of(&normal).refuses_turns(now()));
  let full = reading_of(&snapshot(&Value::Null));
  assert!(full.refuses_turns(now()));
  assert!(!full.refuses_turns(at(3_600)), "the window reset");
  let sparse = json!({"rateLimitReachedType": "rate_limit_reached"});
  assert!(
    reading_of(&sparse).refuses_turns(now()),
    "a refusal that names no window and no credits"
  );
}

/// The updates App Server's `line` gives a fresh stream.
fn updates(line: &Value) -> Vec<Update> {
  AppServerStream::default()
    .parse(&line.to_string())
    .expect("App Server message")
}

fn failures(updates: &[Update]) -> Vec<CodexFailure> {
  updates
    .iter()
    .filter_map(|update| match update {
      Update::Failure(failure) => Some(*failure),
      _ => None,
    })
    .collect()
}

fn failed_turn(error: &Value) -> Value {
  json!({"method": "turn/completed", "params": {"turn": {"id": "turn-1", "status": "failed", "error": error}}})
}

#[test]
fn plan_limit_credits_codex_names_why_a_turn_failed() {
  let limit = failed_turn(&json!({"message": "You've hit your usage limit.", "codexErrorInfo": "usageLimitExceeded"}));
  assert_eq!(failures(&updates(&limit)), [CodexFailure::UsageLimit]);
  let other = failed_turn(
    &json!({"message": "stream disconnected", "codexErrorInfo": {"httpConnectionFailed": {"httpStatusCode": 502}}}),
  );
  assert_eq!(failures(&updates(&other)), [CodexFailure::Other]);
  let unnamed = failed_turn(&json!({"message": "Codex failed"}));
  assert!(failures(&updates(&unnamed)).is_empty(), "no reason is invented");
  let completed = json!({"method": "turn/completed", "params": {"turn": {"id": "turn-1", "status": "completed"}}});
  assert!(failures(&updates(&completed)).is_empty());
  let error = |will_retry: bool| json!({"method": "error", "params": {"willRetry": will_retry, "error": {"message": "limit", "codexErrorInfo": "usageLimitExceeded"}}});
  assert_eq!(failures(&updates(&error(false))), [CodexFailure::UsageLimit]);
  assert!(
    failures(&updates(&error(true))).is_empty(),
    "an error Codex retries ends nothing yet"
  );
}
