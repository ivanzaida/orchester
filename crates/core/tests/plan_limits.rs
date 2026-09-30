use orchester_core::{
  connection::{
    AgentProvider,
    app_server::{AppServerStream, Update},
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanReadings, claude_limits, codex_limits},
    stream::{AgentEvent, AgentStream, OutputStream},
  },
  time::Timestamp,
};
use serde_json::json;

const CODEX: &str = include_str!("fixtures/codex/plan-limits.jsonl");
const CLAUDE: &str = include_str!("fixtures/claude/plan-limits.jsonl");
fn now() -> Timestamp {
  Timestamp::from_unix_seconds(1790682000)
}
fn parsed(provider: AgentProvider, line: &str, at: Timestamp) -> LimitReading {
  let events = AgentStream::new(provider).parse(OutputStream::Stdout, line);
  let [AgentEvent::PlanLimits { limits }] = events.as_slice() else {
    panic!("expected limits, got {events:?}");
  };
  LimitReading {
    provider,
    at,
    limits: limits.clone(),
  }
}

#[test]
fn plan_limits_captured_codex_session_and_claude_stream_events() {
  let captured = parsed(
    AgentProvider::CodexCli,
    include_str!("fixtures/codex/captured-plan-limits.jsonl"),
    now(),
  );
  assert_eq!(captured.fullest(now()).expect("captured window").percent(), 82.0);
  assert_eq!(
    captured.limits.credits.expect("captured credits").balance.as_deref(),
    Some("62500")
  );
  let codex = parsed(AgentProvider::CodexCli, CODEX, now());
  assert_eq!(codex.limits.plan.as_deref(), Some("pro"));
  assert_eq!(codex.fullest(now()).expect("window").percent(), 65.0);
  assert_eq!(codex.fullest(now()).expect("window").minutes(), 10080);
  assert!(!codex.limits.credits.expect("credits").has_credits);
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
  let limits = codex_limits(
    &json!({"primary":{"used_percent":25,"window_minutes":300},"rate_limit_reached_type":"provider_limit"}),
  )
  .expect("provider state");
  assert_eq!(limits.windows[0].state(now()), LimitState::Reached);
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
  let event = json!({"method":"account/rateLimits/updated", "params":{"rateLimits":{"primary":{"usedPercent":100,"windowDurationMins":300,"resetsAt":1790928000},"rateLimitReachedType":"primary"}}});
  let updates = AppServerStream::default()
    .parse(&event.to_string())
    .expect("notification");
  let [Update::Activity(AgentEvent::PlanLimits { limits })] = updates.as_slice() else {
    panic!("limits notification");
  };
  assert_eq!(limits.windows[0].state(now()), LimitState::Reached);
  assert!(claude_limits(&json!({"rateLimitType":"unknown", "status":"allowed", "utilization":0.2})).is_none());
  assert!(codex_limits(&json!({"primary":{"used_percent":-1,"window_minutes":300}})).is_none());
  let events =
    AgentStream::new(AgentProvider::ClaudeCodeCli).parse(OutputStream::Stdout, "{\"type\":\"rate_limit_event\"}");
  assert!(
    matches!(&events[0], AgentEvent::Raw { .. }),
    "unreadable data stays raw"
  );
}
