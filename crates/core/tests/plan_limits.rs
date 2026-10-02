use orchester_core::{
  connection::{
    AgentProvider,
    app_server::{AppServerStream, Update},
    plan_limits::{
      Blocked, InvalidWindow, LimitBlock, LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings,
      claude_limits, codex_limits,
    },
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
  assert_eq!(codex.fullest(now()).expect("window").percent(), Some(65.0));
  assert_eq!(codex.fullest(now()).expect("window").minutes(), 10080);
  let credits = codex.limits.credits.expect("credits");
  assert!(!credits.has_credits);
  assert_eq!(credits.balance.as_deref(), Some("0"));
  let claude = parsed(AgentProvider::ClaudeCodeCli, CLAUDE, now());
  assert_eq!(claude.fullest(now()).expect("window").percent(), Some(96.0));
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
    let window = LimitWindow::new(300, Some(percent), Some(now()), reported, extra).expect("window");
    assert_eq!(
      window.state(Timestamp::from_unix_millis(now().unix_millis() - 1)),
      state
    );
    assert_eq!(window.state(now()), LimitState::Normal);
    assert!(window.expired(now()));
  }
  assert_eq!(
    LimitWindow::new(0, Some(0.0), None, LimitState::Normal, false),
    Err(InvalidWindow)
  );
  assert_eq!(
    LimitWindow::new(300, Some(f64::NAN), None, LimitState::Normal, false),
    Err(InvalidWindow)
  );
  let over = LimitWindow::new(300, Some(112.5), None, LimitState::Normal, false).expect("past the limit");
  assert_eq!((over.percent(), over.state(now())), (Some(100.0), LimitState::Reached));
  let unknown = LimitWindow::new(300, None, None, LimitState::Reached, false).expect("state only");
  assert_eq!((unknown.percent(), unknown.state(now())), (None, LimitState::Reached));
  assert!(
    serde_json::from_value::<LimitWindow>(json!({"minutes":0,"used":50,"resets":null,"state":"Normal","extra":false}))
      .is_err()
  );
  let limits = codex_limits(&json!({
    "primary":{"usedPercent":25,"windowDurationMins":300},
    "secondary":{"usedPercent":40,"windowDurationMins":10080},
    "rateLimitReachedType":"rate_limit_reached"}))
  .expect("provider state");
  assert_eq!(
    limits.blocked,
    Some(Blocked {
      reason: LimitBlock::Window,
      window: Some(10080)
    }),
    "the reason names no window; it applies to the fullest one of the update"
  );
  assert!(
    limits
      .windows
      .iter()
      .all(|window| window.state(now()) == LimitState::Normal),
    "a window's own state stays what the CLI said about it"
  );
  let reading = LimitReading {
    provider: AgentProvider::CodexCli,
    at: now(),
    limits,
  };
  assert_eq!(
    reading.window_state(&reading.limits.windows[0], now()),
    LimitState::Normal
  );
  assert_eq!(
    reading.window_state(&reading.limits.windows[1], now()),
    LimitState::Reached
  );
}

#[test]
fn plan_limits_codex_block_reasons_and_window_lifts() {
  let reset = Timestamp::from_unix_millis(now().unix_millis() + 3_600_000);
  let after = Timestamp::from_unix_millis(reset.unix_millis() + 1);
  for (reason, block, lifts) in [
    ("rate_limit_reached", LimitBlock::Window, true),
    ("workspace_member_credits_depleted", LimitBlock::Credits, false),
    ("workspace_owner_credits_depleted", LimitBlock::Credits, false),
    ("workspace_owner_usage_limit_reached", LimitBlock::Workspace, false),
    ("workspace_member_usage_limit_reached", LimitBlock::Workspace, false),
    ("something_new", LimitBlock::Other, false),
  ] {
    let payload = json!({"primary":{"usedPercent":40,"windowDurationMins":300,"resetsAt":reset.unix_millis()/1000},"rateLimitReachedType":reason});
    let reading = parsed(AgentProvider::CodexCli, &codex_update(&payload), now());
    assert_eq!(reading.limits.blocked.map(|blocked| blocked.reason), Some(block));
    assert_eq!(reading.state(now()), LimitState::Reached, "{reason}");
    let expected = if lifts { LimitState::Normal } else { LimitState::Reached };
    assert_eq!(reading.state(after), expected, "{reason} after the window reset");
  }
  let mut readings = PlanReadings::default();
  readings.observe(parsed(AgentProvider::CodexCli, CODEX, now()));
  let later = Timestamp::from_unix_millis(now().unix_millis() + 60_000);
  let blocked = json!({"rateLimitReachedType":"workspace_owner_credits_depleted"});
  assert!(readings.observe(parsed(AgentProvider::CodexCli, &codex_update(&blocked), later)));
  let reading = readings.get(AgentProvider::CodexCli).expect("Codex");
  assert_eq!(
    reading.limits.blocked.map(|blocked| blocked.reason),
    Some(LimitBlock::Credits),
    "a block without a window is kept"
  );
  assert_eq!(reading.fullest(later).expect("kept window").percent(), Some(65.0));
  let credits = json!({"credits":{"hasCredits":true,"unlimited":false,"balance":"50"}});
  let even_later = Timestamp::from_unix_millis(later.unix_millis() + 60_000);
  assert!(readings.observe(parsed(AgentProvider::CodexCli, &codex_update(&credits), even_later)));
  let reading = readings.get(AgentProvider::CodexCli).expect("Codex");
  assert_eq!(
    reading.limits.blocked.map(|blocked| blocked.reason),
    Some(LimitBlock::Credits),
    "credits alone say nothing of the block"
  );
  assert_eq!(
    reading
      .limits
      .credits
      .as_ref()
      .and_then(|credits| credits.balance.as_deref()),
    Some("50")
  );
  let fresh = json!({"primary":{"usedPercent":10,"windowDurationMins":300}});
  let latest = Timestamp::from_unix_millis(even_later.unix_millis() + 60_000);
  assert!(readings.observe(parsed(AgentProvider::CodexCli, &codex_update(&fresh), latest)));
  let reading = readings.get(AgentProvider::CodexCli).expect("Codex");
  assert_eq!(
    reading.limits.blocked, None,
    "a window update without a block clears it"
  );
  assert_eq!(reading.state(latest), LimitState::Normal);
}

#[test]
fn plan_limits_fullest_percentage_is_separate_from_the_warning_window() {
  let reading = LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    at: now(),
    limits: orchester_core::connection::plan_limits::PlanLimits {
      windows: vec![
        LimitWindow::new(300, Some(20.0), None, LimitState::Near, false).expect("warning"),
        LimitWindow::new(10080, Some(70.0), None, LimitState::Normal, false).expect("fullest"),
      ],
      plan: None,
      credits: None,
      blocked: None,
    },
  };
  assert_eq!(reading.fullest(now()).expect("fullest").percent(), Some(70.0));
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
  assert_eq!(reading.fullest(now()).expect("fullest").percent(), Some(96.0));
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
  assert!(codex_limits(&json!({"primary":{"usedPercent":-1,"windowDurationMins":300}})).is_none());
  let over = codex_limits(&json!({"primary":{"usedPercent":130,"windowDurationMins":300}})).expect("past the limit");
  assert_eq!(over.windows[0].percent(), Some(100.0));
  assert_eq!(over.windows[0].state(now()), LimitState::Reached);
}

#[test]
fn plan_limits_claude_events_never_reach_the_transcript() {
  for line in [
    r#"{"type":"rate_limit_event"}"#,
    r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}"#,
    r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed_warning","rateLimitType":null}}"#,
    r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","rateLimitType":"seven_day_opus","utilization":1}}"#,
    r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"seven_day_sonnet","utilization":0.4}}"#,
    r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"seven_day_overage_included"}}"#,
    r#"{"type":"rate_limit_event","rate_limit_info":{"status":"unknown","rateLimitType":"five_hour"}}"#,
    r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"five_hour","utilization":"high"}}"#,
  ] {
    let events = ClaudeStream::default().parse(OutputStream::Stdout, line);
    assert!(events.is_empty(), "{line} gave {events:?}");
  }
  let state_only = claude_limits(&json!({"status":"rejected","rateLimitType":"five_hour"})).expect("rejected window");
  assert_eq!(state_only.windows[0].percent(), None, "no invented percentage");
  assert_eq!(state_only.windows[0].state(now()), LimitState::Reached);
  let unknown_window = claude_limits(&json!({"status":"rejected","rateLimitType":null})).expect("rejected");
  assert!(unknown_window.windows.is_empty());
  assert_eq!(
    unknown_window.blocked,
    Some(Blocked {
      reason: LimitBlock::Window,
      window: None
    })
  );
  let reading = LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    at: now(),
    limits: unknown_window,
  };
  assert_eq!(reading.state(now()), LimitState::Reached);
  let over =
    claude_limits(&json!({"status":"allowed","rateLimitType":"seven_day","utilization":1.04})).expect("past the limit");
  assert_eq!(over.windows[0].percent(), Some(100.0));
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
        (window.percent(), reading.window_state(window, later), window.resets()),
        (Some(percent), state, Some(reset))
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
  assert_eq!(reading.fullest(later).expect("fullest").percent(), Some(65.0));
}

fn claude_event(info: &serde_json::Value) -> String {
  json!({"type":"rate_limit_event","rate_limit_info":info}).to_string()
}

fn at(minutes: i64) -> Timestamp {
  Timestamp::from_unix_millis(now().unix_millis() + minutes * 60_000)
}

#[test]
fn plan_limits_block_without_its_window_over_an_expired_retained_window() {
  let mut readings = PlanReadings::default();
  let weekly =
    json!({"status":"allowed","rateLimitType":"seven_day","utilization":0.96,"resetsAt":at(30).unix_millis()/1000});
  readings.observe(parsed(AgentProvider::ClaudeCodeCli, &claude_event(&weekly), now()));
  let rejected = json!({"status":"rejected","rateLimitType":null});
  assert!(readings.observe(parsed(AgentProvider::ClaudeCodeCli, &claude_event(&rejected), at(60))));
  let reading = readings.get(AgentProvider::ClaudeCodeCli).expect("Claude");
  assert_eq!(reading.state(at(60)), LimitState::Reached, "the fresh rejection holds");
  assert_eq!(
    reading.block_beyond_reset(),
    Some(LimitBlock::Window),
    "with no reset to name"
  );
  let codex = json!({"rateLimitReachedType":"rate_limit_reached"});
  let mut readings = PlanReadings::default();
  readings.observe(parsed(AgentProvider::CodexCli, CODEX, now()));
  assert!(readings.observe(parsed(AgentProvider::CodexCli, &codex_update(&codex), at(1))));
  let reading = readings.get(AgentProvider::CodexCli).expect("Codex");
  assert_eq!(reading.state(at(1)), LimitState::Reached);
  assert_eq!(reading.block_beyond_reset(), Some(LimitBlock::Window));
  let kept = reading.fullest(at(1)).expect("retained window");
  assert_eq!(
    (kept.state(at(1)), reading.window_state(kept, at(1))),
    (LimitState::Normal, LimitState::Normal),
    "the block is not pinned on the retained window"
  );
}

#[test]
fn plan_limits_block_without_its_window_is_not_kept_on_a_live_window() {
  let mut readings = PlanReadings::default();
  let weekly =
    json!({"status":"allowed","rateLimitType":"seven_day","utilization":0.4,"resetsAt":at(600).unix_millis()/1000});
  readings.observe(parsed(AgentProvider::ClaudeCodeCli, &claude_event(&weekly), now()));
  let rejected = json!({"status":"rejected"});
  readings.observe(parsed(AgentProvider::ClaudeCodeCli, &claude_event(&rejected), at(1)));
  let reading = readings.get(AgentProvider::ClaudeCodeCli).expect("Claude");
  assert_eq!(reading.state(at(1)), LimitState::Reached);
  let limiting = reading.limiting(at(1)).expect("weekly");
  assert_eq!(
    reading.window_state(limiting, at(1)),
    LimitState::Normal,
    "no reset is claimed for it"
  );
  let five =
    json!({"status":"allowed","rateLimitType":"five_hour","utilization":0.1,"resetsAt":at(120).unix_millis()/1000});
  readings.observe(parsed(AgentProvider::ClaudeCodeCli, &claude_event(&five), at(2)));
  let reading = readings.get(AgentProvider::ClaudeCodeCli).expect("Claude");
  assert_eq!(reading.state(at(2)), LimitState::Normal);
  assert!(
    reading
      .limits
      .windows
      .iter()
      .all(|window| reading.window_state(window, at(2)) == LimitState::Normal),
    "the weekly window is not left reached"
  );
}

#[test]
fn plan_limits_status_only_report_keeps_a_use_it_does_not_contradict() {
  let reset = at(120).unix_millis() / 1000;
  let mut readings = PlanReadings::default();
  let known = json!({"status":"allowed","rateLimitType":"five_hour","utilization":0.4,"resetsAt":reset});
  readings.observe(parsed(AgentProvider::ClaudeCodeCli, &claude_event(&known), now()));
  let status = json!({"status":"allowed","rateLimitType":"five_hour","resetsAt":reset});
  readings.observe(parsed(AgentProvider::ClaudeCodeCli, &claude_event(&status), at(5)));
  let reading = readings.get(AgentProvider::ClaudeCodeCli).expect("Claude");
  let window = &reading.limits.windows[0];
  assert_eq!(window.percent(), Some(40.0));
  assert_eq!(window.reported_at(), Some(now()), "the use keeps its own age");
  let warning = json!({"status":"allowed_warning","rateLimitType":"five_hour","resetsAt":reset});
  readings.observe(parsed(AgentProvider::ClaudeCodeCli, &claude_event(&warning), at(6)));
  let window = &readings
    .get(AgentProvider::ClaudeCodeCli)
    .expect("Claude")
    .limits
    .windows[0];
  assert_eq!(window.percent(), None, "a warning contradicts 40 %");
  assert_eq!(window.state(at(6)), LimitState::Near);
  let next = json!({"status":"allowed","rateLimitType":"five_hour","utilization":0.4,"resetsAt":reset});
  readings.observe(parsed(AgentProvider::ClaudeCodeCli, &claude_event(&next), at(7)));
  let moved = json!({"status":"allowed","rateLimitType":"five_hour","resetsAt":reset + 18_000});
  readings.observe(parsed(AgentProvider::ClaudeCodeCli, &claude_event(&moved), at(8)));
  let window = &readings
    .get(AgentProvider::ClaudeCodeCli)
    .expect("Claude")
    .limits
    .windows[0];
  assert_eq!(window.percent(), None, "a new period does not inherit the old use");
}
