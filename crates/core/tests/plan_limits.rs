use orchester_core::{
  connection::{
    AgentProvider,
    app_server::{AppServerStream, Update},
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings, claude_limits, codex_limits},
    stream::{AgentEvent, ClaudeStream, OutputStream},
  },
  time::Timestamp,
};
use serde_json::json;

const CODEX: &str = include_str!("fixtures/codex/plan-limits.jsonl");
const CLAUDE: &str = include_str!("fixtures/claude/plan-limits.jsonl");
fn now() -> Timestamp {
  Timestamp::from_unix_seconds(1790682000)
}
/// What the CLI's own channel reports: Codex App Server notifications, Claude Code `stream-json`.
fn reported(provider: AgentProvider, line: &str) -> Vec<PlanLimits> {
  let events = match provider {
    AgentProvider::CodexCli => AppServerStream::default()
      .parse(line)
      .expect("App Server message")
      .into_iter()
      .filter_map(|update| match update {
        Update::Activity(event) => Some(event),
        _ => None,
      })
      .collect(),
    _ => ClaudeStream::default().parse(OutputStream::Stdout, line),
  };
  events
    .into_iter()
    .filter_map(|event| match event {
      AgentEvent::PlanLimits { limits } => Some(limits),
      _ => None,
    })
    .collect()
}

fn parsed(provider: AgentProvider, line: &str, at: Timestamp) -> LimitReading {
  let reported = reported(provider, line);
  let [limits] = reported.as_slice() else {
    panic!("expected limits, got {reported:?}");
  };
  LimitReading {
    provider,
    at,
    limits: limits.clone(),
  }
}

fn codex_update(limits: &serde_json::Value) -> String {
  json!({"method": "account/rateLimits/updated", "params": {"rateLimits": limits}}).to_string()
}

#[test]
fn plan_limits_codex_app_server_and_claude_stream_events() {
  let codex = parsed(AgentProvider::CodexCli, CODEX, now());
  assert_eq!(codex.limits.plan.as_deref(), Some("pro"));
  assert_eq!(codex.fullest(now()).expect("window").percent(), 65.0);
  assert_eq!(codex.fullest(now()).expect("window").minutes(), 10080);
  let credits = codex.limits.credits.expect("credits");
  assert!(!credits.has_credits);
  assert_eq!(credits.balance.as_deref(), Some("0"));
  let claude = parsed(AgentProvider::ClaudeCodeCli, CLAUDE, now());
  assert_eq!(claude.fullest(now()).expect("window").percent(), 96.0);
  assert_eq!(claude.fullest(now()).expect("window").state(now()), LimitState::Near);
  assert!(claude.limits.plan.is_none());
}

#[test]
fn plan_limits_warning_full_extra_and_expired_semantics() {
  for (percent, reported, extra, state) in [
    (79.0, LimitState::Normal, false, LimitState::Normal),
    (80.0, LimitState::Normal, false, LimitState::Near),
    (12.0, LimitState::Near, false, LimitState::Near),
    (100.0, LimitState::Normal, false, LimitState::Reached),
    (70.0, LimitState::Reached, false, LimitState::Reached),
    (100.0, LimitState::Reached, true, LimitState::Near),
  ] {
    let window = LimitWindow::new(300, percent, Some(now()), reported, extra).expect("window");
    assert_eq!(
      window.state(Timestamp::from_unix_millis(now().unix_millis() - 1)),
      state
    );
    assert_eq!(window.state(now()), LimitState::Normal);
    assert!(window.expired(now()));
  }
  assert!(LimitWindow::new(0, 0.0, None, LimitState::Normal, false).is_none());
  assert!(LimitWindow::new(300, f64::NAN, None, LimitState::Normal, false).is_none());
  assert!(
    serde_json::from_value::<LimitWindow>(json!({"minutes":0,"used":50,"resets":null,"state":"Normal","extra":false}))
      .is_err()
  );
  let limits = codex_limits(&json!({
    "primary":{"usedPercent":25,"windowDurationMins":300},
    "secondary":{"usedPercent":40,"windowDurationMins":10080},
    "rateLimitReachedType":"workspace_member_usage_limit_reached"}))
  .expect("provider state");
  assert_eq!(limits.windows[0].state(now()), LimitState::Normal);
  assert_eq!(
    limits.windows[1].state(now()),
    LimitState::Reached,
    "the reason names no window; the fullest one is reached"
  );
}

#[test]
fn plan_limits_fullest_percentage_is_separate_from_the_warning_window() {
  let reading = LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    at: now(),
    limits: orchester_core::connection::plan_limits::PlanLimits {
      windows: vec![
        LimitWindow::new(300, 20.0, None, LimitState::Near, false).expect("warning"),
        LimitWindow::new(10080, 70.0, None, LimitState::Normal, false).expect("fullest"),
      ],
      plan: None,
      credits: None,
    },
  };
  assert_eq!(reading.fullest(now()).expect("fullest").percent(), 70.0);
  assert_eq!(reading.limiting(now()).expect("warning").minutes(), 300);
}

#[test]
fn plan_limits_newest_across_projects_and_partial_claude_windows() {
  let mut readings = PlanReadings::default();
  assert!(readings.observe(parsed(AgentProvider::ClaudeCodeCli, CLAUDE, now())));
  let later = Timestamp::from_unix_millis(now().unix_millis() + 120_000);
  let five = json!({"type":"rate_limit_event", "rate_limit_info": {"status":"allowed", "rateLimitType":"five_hour", "utilization":0.12, "resetsAt":1790928000}});
  assert!(readings.observe(parsed(AgentProvider::ClaudeCodeCli, &five.to_string(), later)));
  assert!(!readings.observe(parsed(AgentProvider::ClaudeCodeCli, CLAUDE, now())));
  let reading = readings.get(AgentProvider::ClaudeCodeCli).expect("Claude");
  assert_eq!(reading.at, later);
  assert_eq!(reading.limits.windows.len(), 2);
  let weekly = reading
    .limits
    .windows
    .iter()
    .find(|window| window.minutes() == 10080)
    .expect("weekly");
  let five = reading
    .limits
    .windows
    .iter()
    .find(|window| window.minutes() == 300)
    .expect("five-hour");
  assert_eq!(
    weekly.reported_at(),
    Some(now()),
    "retained window keeps its report time"
  );
  assert_eq!(five.reported_at(), Some(later), "new window has its own time");
  let round_trip: LimitReading =
    serde_json::from_str(&serde_json::to_string(reading).expect("serialize")).expect("deserialize");
  assert_eq!(&round_trip, reading, "window timestamps survive IPC serialization");
  assert_eq!(reading.fullest(now()).expect("fullest").percent(), 96.0);
  assert!(readings.observe(parsed(AgentProvider::CodexCli, CODEX, later)));
  assert_eq!(readings.get(AgentProvider::CodexCli).expect("Codex").at, later);
  assert!(
    PlanReadings::default().get(AgentProvider::CodexCli).is_none(),
    "new session starts empty"
  );
}

#[test]
fn plan_limits_app_server_notifications_and_invalid_unknown_events() {
  let event = codex_update(
    &json!({"primary":{"usedPercent":100,"windowDurationMins":300,"resetsAt":1790928000},"rateLimitReachedType":"rate_limit_reached"}),
  );
  let updates = AppServerStream::default().parse(&event).expect("notification");
  let [Update::Activity(AgentEvent::PlanLimits { limits })] = updates.as_slice() else {
    panic!("limits notification");
  };
  assert_eq!(limits.windows[0].state(now()), LimitState::Reached);
  let model = json!({"limitId":"codex_other","primary":{"usedPercent":90,"windowDurationMins":300}});
  assert!(
    codex_limits(&model).is_none(),
    "a single model's bucket is not the plan"
  );
  assert!(claude_limits(&json!({"rateLimitType":"unknown", "status":"allowed", "utilization":0.2})).is_none());
  assert!(codex_limits(&json!({"primary":{"usedPercent":-1,"windowDurationMins":300}})).is_none());
  let events = ClaudeStream::default().parse(OutputStream::Stdout, "{\"type\":\"rate_limit_event\"}");
  assert!(
    matches!(&events[0], AgentEvent::Raw { .. }),
    "unreadable data stays raw"
  );
}

#[test]
fn plan_limits_malformed_newer_codex_event_preserves_state_age_and_expiry() {
  let later = Timestamp::from_unix_millis(now().unix_millis() + 120_000);
  let reset = Timestamp::from_unix_millis(now().unix_millis() + 3_600_000);
  for (percent, reached, state) in [
    (65.0, None, LimitState::Normal),
    (85.0, None, LimitState::Near),
    (70.0, Some("rate_limit_reached"), LimitState::Reached),
  ] {
    let valid = json!({"primary":{"usedPercent":percent,"windowDurationMins":300,"resetsAt":reset.unix_millis()/1000},"rateLimitReachedType":reached});
    let mut readings = PlanReadings::default();
    readings.observe(parsed(AgentProvider::CodexCli, &codex_update(&valid), now()));
    let retained = readings.get(AgentProvider::CodexCli).expect("reading").clone();
    for payload in [
      json!(null),
      json!(true),
      json!(12),
      json!("unknown"),
      json!([]),
      json!({}),
      json!({"primary":null,"secondary":null}),
      json!({"primary":{}}),
      json!({"primary":{"usedPercent":10}}),
      json!({"credits":{"hasCredits":true,"unlimited":false,"balance":"50"}}),
      json!({"primary":{"usedPercent":10,"windowDurationMins":300,"resetsAt":"bad"}}),
      json!({"primary":{"usedPercent":10,"windowDurationMins":300},"credits":false}),
      json!({"primary":{"usedPercent":10,"windowDurationMins":300},"rateLimitReachedType":true}),
      json!({"primary":{"usedPercent":10,"windowDurationMins":300},"planType":[]}),
    ] {
      assert!(codex_limits(&payload).is_none(), "malformed payload: {payload}");
      for limits in reported(AgentProvider::CodexCli, &codex_update(&payload)) {
        readings.observe(LimitReading {
          provider: AgentProvider::CodexCli,
          at: later,
          limits,
        });
      }
      let reading = readings.get(AgentProvider::CodexCli).expect("retained reading");
      assert_eq!(reading, &retained, "a newer malformed event is not a snapshot");
      let window = reading.fullest(later).expect("unexpired window");
      assert_eq!(
        (window.percent(), window.state(later), window.resets()),
        (percent, state, Some(reset))
      );
      assert_eq!(window.reported_at(), Some(now()));
      assert_eq!(
        later.unix_millis() - reading.at.unix_millis(),
        120_000,
        "age keeps advancing"
      );
      assert!(
        reading.fullest(reset).is_none(),
        "the original reset still expires the reading"
      );
      assert_eq!(reading.limits.windows[0].state(reset), LimitState::Normal);
    }
    assert!(
      reported(
        AgentProvider::CodexCli,
        r#"{"method":"account/rateLimits/updated","params":{}}"#
      )
      .is_empty()
    );
  }
}

#[test]
fn plan_limits_sparse_codex_update_keeps_what_it_leaves_out() {
  let mut readings = PlanReadings::default();
  readings.observe(parsed(AgentProvider::CodexCli, CODEX, now()));
  let later = Timestamp::from_unix_millis(now().unix_millis() + 60_000);
  let sparse = json!({"primary":{"usedPercent":30,"windowDurationMins":300},"secondary":{"usedPercent":99,"windowDurationMins":null},"planType":null,"credits":null});
  assert!(readings.observe(parsed(AgentProvider::CodexCli, &codex_update(&sparse), later)));
  let reading = readings.get(AgentProvider::CodexCli).expect("Codex");
  let minutes: Vec<u64> = reading.limits.windows.iter().map(LimitWindow::minutes).collect();
  assert_eq!(minutes, [300, 10080], "a window without its duration is left out");
  assert_eq!(reading.limits.plan.as_deref(), Some("pro"));
  assert!(reading.limits.credits.is_some());
  assert_eq!(reading.fullest(later).expect("fullest").percent(), 65.0);
}
