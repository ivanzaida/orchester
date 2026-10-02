//! The plan limits item (`XCXvw`, states M–R in `poPso`).

use orchester_desktop::plan_limits::Connections;

use super::*;

pub(super) fn cli(provider: orchester_core::connection::AgentProvider) -> orchester_core::connection::AgentConnection {
  use orchester_core::{connection::AgentConnection, id::ConnectionId, name::Name, time::Timestamp};
  AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Local agent").expect("name"),
    provider,
    std::env::current_dir()
      .expect("workspace")
      .join(provider.program_name().expect("an agent CLI")),
    Timestamp::now(),
  )
  .expect("connection")
}

fn limits_readings(percent: f64, extra: bool, expired: bool) -> orchester_core::connection::plan_limits::PlanReadings {
  use orchester_core::{
    connection::{
      AgentProvider,
      plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
    },
    time::Timestamp,
  };
  let now = Timestamp::now();
  let reset = Timestamp::from_unix_millis(now.unix_millis() + if expired { -1000 } else { 7_200_000 });
  let mut readings = PlanReadings::default();
  for provider in AgentProvider::CLI {
    readings.observe(LimitReading {
      provider,
      at: now,
      limits: PlanLimits {
        windows: vec![
          LimitWindow::new(
            10080,
            Some(if provider == AgentProvider::CodexCli {
              percent
            } else {
              100.0
            }),
            Some(reset),
            LimitState::Normal,
            provider == AgentProvider::ClaudeCodeCli && extra,
          )
          .expect("window"),
        ],
        plan: None,
        credits: None,
        blocked: None,
      },
    });
  }
  readings
}

fn with_limits(
  width: u32,
  readings: orchester_core::connection::plan_limits::PlanReadings,
  connections: Vec<orchester_core::connection::AgentConnection>,
) -> (App, Tree) {
  with_connections(width, readings, Connections::Ready(connections))
}

fn with_connections(
  width: u32,
  readings: orchester_core::connection::plan_limits::PlanReadings,
  connections: Connections,
) -> (App, Tree) {
  let mut bar = project(Some(2));
  bar.project.as_mut().expect("project").connections = connections;
  with_bar(width, readings, bar)
}

pub(super) fn with_bar(
  width: u32,
  readings: orchester_core::connection::plan_limits::PlanReadings,
  bar: StatusBarProps,
) -> (App, Tree) {
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("locale");
  let mut tree = Tree::new();
  tree.resize(width, 960);
  tree.mount_root::<Shell>(
    &mut app,
    ShellProps {
      bar,
      log: ActivityLog::default(),
      mcp: McpStatus::Listening {
        port: std::num::NonZeroU16::new(47291).expect("port"),
      },
      readings: orchester_desktop::plan_limits::Readings(readings),
    },
  );
  for _ in 0..3 {
    tree.pass_headless(&mut app);
  }
  (app, tree)
}

#[test]
fn status_bar_plan_limits_960_preserves_limits_and_drops_labels_when_reached() {
  use orchester_core::connection::AgentProvider;
  for reached in [false, true] {
    let readings = limits_readings(if reached { 100.0 } else { 65.0 }, true, false);
    let (mut app, mut tree) = with_limits(960, readings, AgentProvider::CLI.into_iter().map(cli).collect());
    assert!(tree.get_element_by_id("status-version").is_none());
    assert!(tree.get_element_by_id("status-tokens").is_none());
    let limits = bounds(&mut tree, "status-plan-limits");
    let runner = bounds(&mut tree, "status-runner");
    let mcp = bounds(&mut tree, "status-mcp");
    assert!(runner.x + runner.width <= limits.x);
    assert!(limits.x + limits.width <= mcp.x);
    assert!(mcp.x + mcp.width <= 960.0);
    assert_eq!(
      item_texts(&mut tree, "status-agents")
        .iter()
        .any(|text| text == "Agents"),
      !reached
    );
    hover(&mut app, &mut tree, "status-plan-limits");
    let hint = bounds(&mut tree, "status-plan-limits-hint");
    assert!(hint.x >= 0.0 && hint.x + hint.width <= 960.0);
    let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
    assert!(copy.contains("Extra usage"));
    assert!(copy.contains("Not reported yet"));
    assert!(copy.contains("From the last Codex turn"));
  }
}

#[test]
fn status_bar_plan_limits_unknown_signed_out_and_expired() {
  use orchester_core::{
    connection::{AgentProvider, CheckResult, ConnectionCheck, ModelCatalog, SignIn},
    time::Timestamp,
  };
  let codex = cli(AgentProvider::CodexCli);
  let mut claude = cli(AgentProvider::ClaudeCodeCli);
  claude
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "2.1.89".into(),
        sign_in: SignIn::SignedOut,
        catalog: ModelCatalog::new(orchester_core::connection::CatalogSource::DocumentedAliases, Vec::new()),
      },
    })
    .expect("an agent CLI's check");
  let (mut app, mut tree) = with_limits(1440, Default::default(), vec![codex, claude]);
  assert_eq!(
    item_texts(&mut tree, "status-plan-limits"),
    ["Codex", "—", "·", "Claude", "—"]
  );
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
  assert!(copy.contains("They appear after the next Codex turn in any open project."));
  assert!(copy.contains("not signed in on this computer"));
  assert!(copy.contains("Codex Not read yet"));
  assert!(copy.contains("Claude Code Not signed in"));
  assert_eq!(copy.matches("Connection checked").count(), 1);
  assert!(bounds(&mut tree, "status-plan-limits-claude-heading").height < 20.0);
  let (mut app, mut tree) = with_limits(
    1440,
    limits_readings(100.0, false, true),
    vec![cli(AgentProvider::CodexCli)],
  );
  assert_eq!(item_texts(&mut tree, "status-plan-limits"), ["Codex", "—"]);
  hover(&mut app, &mut tree, "status-plan-limits");
  assert!(
    item_texts(&mut tree, "status-plan-limits-hint")
      .join(" ")
      .contains("read again with the next turn")
  );
}

#[test]
fn status_bar_plan_limits_absent_without_connections() {
  let (_, mut tree) = with_limits(960, Default::default(), Vec::new());
  assert!(tree.get_element_by_id("status-plan-limits").is_none());
}

#[test]
fn status_bar_plan_limits_unavailable_names_connection_and_project() {
  use orchester_core::{
    connection::{AgentProvider, CheckResult, ConnectionCheck},
    time::Timestamp,
  };
  let mut connection = cli(AgentProvider::ClaudeCodeCli);
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::CouldNotRun,
    })
    .expect("an agent CLI's check");
  let (mut app, mut tree) = with_limits(1440, Default::default(), vec![connection]);
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
  assert!(copy.contains("Claude Code No working connection"));
  assert!(copy.contains("Local agent · Kontur could not start Claude Code when it was checked at"));
}

#[test]
fn status_bar_plan_limits_partial_claude_keeps_window_ages() {
  use orchester_core::{
    connection::{
      AgentProvider,
      plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
    },
    time::Timestamp,
  };
  let now = Timestamp::now();
  let old = Timestamp::from_unix_millis(now.unix_millis() - 180_000);
  let mut readings = PlanReadings::default();
  for (minutes, percent, at) in [(10080, 42.0, old), (300, 12.0, now)] {
    readings.observe(LimitReading {
      provider: AgentProvider::ClaudeCodeCli,
      at,
      limits: PlanLimits {
        windows: vec![LimitWindow::new(minutes, Some(percent), None, LimitState::Normal, false).expect("window")],
        plan: None,
        credits: None,
        blocked: None,
      },
    });
  }
  let (mut app, mut tree) = with_limits(1440, readings, vec![cli(AgentProvider::ClaudeCodeCli)]);
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint");
  assert!(
    copy
      .iter()
      .any(|line| line.starts_with("5-hour · From the last Claude Code turn") && line.ends_with("just now"))
  );
  assert!(
    copy
      .iter()
      .any(|line| line.starts_with("Weekly · From the last Claude Code turn") && line.ends_with("3 min ago"))
  );
}

#[test]
fn status_bar_plan_limits_headers_order_and_combined_account_rows() {
  use orchester_core::{
    connection::{
      AgentProvider,
      plan_limits::{Credits, LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
    },
    time::Timestamp,
  };
  let now = Timestamp::now();
  let mut readings = PlanReadings::default();
  readings.observe(LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    at: now,
    limits: PlanLimits {
      windows: vec![LimitWindow::new(10080, Some(96.0), None, LimitState::Near, false).expect("window")],
      plan: None,
      credits: None,
      blocked: None,
    },
  });
  let (mut app, mut tree) = with_limits(1440, readings, vec![cli(AgentProvider::ClaudeCodeCli)]);
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint");
  assert_eq!(
    &copy[..5],
    [
      "Claude Code",
      "Near the weekly limit",
      "5-hour",
      "Not reported yet",
      "Weekly"
    ]
  );

  let mut readings = PlanReadings::default();
  for (provider, minutes, percent, extra, plan, credits) in [
    (
      AgentProvider::CodexCli,
      10080,
      38.0,
      false,
      Some("plus".to_owned()),
      Some(Credits {
        has_credits: true,
        unlimited: false,
        balance: Some("1250".into()),
      }),
    ),
    (AgentProvider::ClaudeCodeCli, 300, 100.0, true, None, None),
  ] {
    readings.observe(LimitReading {
      provider,
      at: now,
      limits: PlanLimits {
        windows: vec![LimitWindow::new(minutes, Some(percent), None, LimitState::Normal, extra).expect("window")],
        plan,
        credits,
        blocked: None,
      },
    });
  }
  let (mut app, mut tree) = with_limits(1440, readings, AgentProvider::CLI.into_iter().map(cli).collect());
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
  assert!(copy.contains("Codex Plus plan"));
  assert!(copy.contains("Credits 1,250 left"));
  assert!(copy.contains("Extra usage In use: turns past the limit use it"));
  assert!(!copy.contains("Near the"));
  assert!(!copy.contains("Not reported yet"));
  assert!(!copy.contains("limit reached"));
}

#[test]
fn status_bar_plan_limits_unreadable_connections_stay_visible() {
  let failed = Connections::Failed(orchester_desktop::messages::interrupted());
  let (mut app, mut tree) = with_connections(1440, Default::default(), failed);
  assert_eq!(item_texts(&mut tree, "status-plan-limits"), ["Plan limits", "—"]);
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
  assert!(copy.contains("connections could not be read, so its plan limits are not shown"));
}

#[test]
fn status_bar_plan_limits_credit_block_names_its_reason_without_a_reset() {
  use orchester_core::{
    connection::{
      AgentProvider,
      plan_limits::{Blocked, LimitBlock, LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
    },
    time::Timestamp,
  };
  let now = Timestamp::now();
  let reset = Timestamp::from_unix_millis(now.unix_millis() + 7_200_000);
  let mut readings = PlanReadings::default();
  readings.observe(LimitReading {
    provider: AgentProvider::CodexCli,
    at: now,
    limits: PlanLimits {
      windows: vec![LimitWindow::new(300, Some(40.0), Some(reset), LimitState::Normal, false).expect("window")],
      plan: None,
      credits: None,
      blocked: Some(Blocked {
        reason: LimitBlock::Credits,
        window: None,
      }),
    },
  });
  let (mut app, mut tree) = with_limits(1440, readings, vec![cli(AgentProvider::CodexCli)]);
  assert_eq!(
    item_texts(&mut tree, "status-plan-limits"),
    ["Codex limit reached", "no credits left"]
  );
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
  assert!(copy.contains("No credits left"));
  assert!(copy.contains("Codex refuses new turns: the workspace has no credits left."));
  assert!(!copy.contains("until"), "a reset does not lift a credit block: {copy}");
}

#[test]
fn status_bar_plan_limits_reached_without_its_use_shows_no_percentage() {
  use orchester_core::{
    connection::{
      AgentProvider,
      plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
    },
    time::Timestamp,
  };
  let now = Timestamp::now();
  let reset = Timestamp::from_unix_millis(now.unix_millis() + 7_200_000);
  let mut readings = PlanReadings::default();
  readings.observe(LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    at: now,
    limits: PlanLimits {
      windows: vec![LimitWindow::new(300, None, Some(reset), LimitState::Reached, false).expect("window")],
      plan: None,
      credits: None,
      blocked: None,
    },
  });
  let (mut app, mut tree) = with_limits(1440, readings, vec![cli(AgentProvider::ClaudeCodeCli)]);
  let item = item_texts(&mut tree, "status-plan-limits");
  assert_eq!(item[0], "Claude limit reached");
  assert!(item[1].starts_with("resets "));
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
  assert!(copy.contains("5-hour — "), "no invented figure: {copy}");
  assert!(copy.contains("Claude refuses new turns until"));
}

#[test]
fn status_bar_plan_limits_percent_column_and_short_reset() {
  use orchester_core::{
    connection::{
      AgentProvider,
      plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
    },
    time::Timestamp,
  };
  let now = Timestamp::now();
  let reset = Timestamp::from_unix_millis(now.unix_millis() + 48 * 60_000 - 1_000);
  let mut readings = PlanReadings::default();
  readings.observe(LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    at: now,
    limits: PlanLimits {
      windows: vec![LimitWindow::new(300, Some(7.0), Some(reset), LimitState::Normal, false).expect("window")],
      plan: None,
      credits: None,
      blocked: None,
    },
  });
  let (mut app, mut tree) = with_limits(1440, readings, vec![cli(AgentProvider::ClaudeCodeCli)]);
  hover(&mut app, &mut tree, "status-plan-limits");
  assert_eq!(bounds(&mut tree, "status-plan-limits-percent").width, 32.0);
  let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
  assert!(copy.contains("7% resets in 48 min"), "{copy}");
}

#[test]
fn status_bar_plan_limits_block_without_its_window_names_no_reset() {
  use orchester_core::{
    connection::{
      AgentProvider,
      plan_limits::{Blocked, LimitBlock, LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
    },
    time::Timestamp,
  };
  let now = Timestamp::now();
  let reset = Timestamp::from_unix_millis(now.unix_millis() + 7_200_000);
  let mut readings = PlanReadings::default();
  readings.observe(LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    at: now,
    limits: PlanLimits {
      windows: vec![LimitWindow::new(10080, Some(40.0), Some(reset), LimitState::Normal, false).expect("window")],
      plan: None,
      credits: None,
      blocked: Some(Blocked {
        reason: LimitBlock::Window,
        window: None,
      }),
    },
  });
  let (mut app, mut tree) = with_limits(1440, readings, vec![cli(AgentProvider::ClaudeCodeCli)]);
  assert_eq!(item_texts(&mut tree, "status-plan-limits"), ["Claude limit reached"]);
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
  assert!(
    copy.contains("Claude refuses new turns; it did not say until when."),
    "{copy}"
  );
  assert!(!copy.contains("until 1") && !copy.contains("until 2") && !copy.contains("until 0"));
}

#[test]
fn status_bar_plan_limits_whole_hours_reset_reads_without_minutes() {
  use orchester_core::{
    connection::{
      AgentProvider,
      plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
    },
    time::Timestamp,
  };
  let now = Timestamp::now();
  let reset = Timestamp::from_unix_millis(now.unix_millis() + 3 * 3_600_000 - 1_000);
  let mut readings = PlanReadings::default();
  readings.observe(LimitReading {
    provider: AgentProvider::CodexCli,
    at: now,
    limits: PlanLimits {
      windows: vec![LimitWindow::new(300, Some(7.0), Some(reset), LimitState::Normal, false).expect("window")],
      plan: None,
      credits: None,
      blocked: None,
    },
  });
  let (mut app, mut tree) = with_limits(1440, readings, vec![cli(AgentProvider::CodexCli)]);
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
  assert!(copy.contains("7% resets in 3 h From"), "{copy}");
}

#[test]
fn status_bar_plan_limits_reached_hint_names_the_reset_day() {
  use orchester_core::{
    connection::{
      AgentProvider,
      plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
    },
    time::Timestamp,
  };
  let now = Timestamp::now();
  let reset = Timestamp::from_unix_millis(now.unix_millis() + 3 * 86_400_000);
  let mut readings = PlanReadings::default();
  readings.observe(LimitReading {
    provider: AgentProvider::CodexCli,
    at: now,
    limits: PlanLimits {
      windows: vec![LimitWindow::new(10080, Some(100.0), Some(reset), LimitState::Normal, false).expect("window")],
      plan: None,
      credits: None,
      blocked: None,
    },
  });
  let (mut app, mut tree) = with_limits(1440, readings, vec![cli(AgentProvider::CodexCli)]);
  let item = item_texts(&mut tree, "status-plan-limits");
  let day = item[1].strip_prefix("resets ").expect("reset").to_owned();
  assert_eq!(day.split(' ').count(), 2, "the bar names the day: {item:?}");
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
  assert!(
    copy.contains(&format!("Codex refuses new turns until {day}.")),
    "{copy}"
  );
}

#[test]
fn status_bar_plan_limits_refusal_without_a_window_shows_its_own_time() {
  use orchester_core::{
    connection::{
      AgentProvider,
      plan_limits::{Blocked, LimitBlock, LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
    },
    time::Timestamp,
  };
  let now = Timestamp::now();
  let earlier = Timestamp::from_unix_millis(now.unix_millis() - 180_000);
  let reset = Timestamp::from_unix_millis(now.unix_millis() + 3 * 86_400_000);
  let mut readings = PlanReadings::default();
  readings.observe(LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    at: earlier,
    limits: PlanLimits {
      windows: vec![LimitWindow::new(10080, Some(100.0), Some(reset), LimitState::Reached, false).expect("window")],
      plan: None,
      credits: None,
      blocked: None,
    },
  });
  readings.observe(LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    at: now,
    limits: PlanLimits {
      windows: Vec::new(),
      plan: None,
      credits: None,
      blocked: Some(Blocked {
        reason: LimitBlock::Window,
        window: None,
      }),
    },
  });
  let (mut app, mut tree) = with_limits(1440, readings, vec![cli(AgentProvider::ClaudeCodeCli)]);
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint");
  let sources: Vec<&String> = copy.iter().filter(|line| line.contains("From the last")).collect();
  assert!(
    sources[0].starts_with("From the last Claude Code turn") && sources[0].ends_with("just now"),
    "{sources:?}"
  );
  assert!(
    sources[1].starts_with("Weekly · From the last Claude Code turn") && sources[1].ends_with("3 min ago"),
    "{sources:?}"
  );
}
