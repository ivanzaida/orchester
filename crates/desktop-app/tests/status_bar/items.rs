//! What the runner, the health dot and its card, and the runs that auto-approve say and ask for (`Status bar ·
//! redesign` `bZVXG`).

use super::*;

fn online_with_slots(slots: u32) -> RunnerFacts {
  RunnerFacts {
    work: Some(WorkSummary {
      agent_slots: NonZeroU32::new(slots).expect("slots"),
      runs: Vec::new(),
    }),
    online_since: Some(SystemTime::now()),
    ..runner(RunnerState::Online)
  }
}

/// Calm (`02 · After · calm` `aexpB`): no project name, counts, MCP address or version; only the dot at the end.
#[test]
fn status_calm_bar_shows_only_the_health_dot() {
  let tasks = ProjectTasks {
    pending_tool_approvals: Arc::new(vec![approval()]),
    ..ProjectTasks::default()
  };
  let mut bar = Bar::of(BarFacts {
    mcp: McpStatus {
      settings: McpSettings::with_window_control(false),
      ..listening()
    },
    ..facts(Some(kontur(online_with_slots(4), tasks)))
  });
  for id in [
    "status-project",
    "status-runner",
    "status-counts",
    "status-agents",
    "status-needs-you",
    "status-approvals",
    "status-mcp",
    "status-version",
    "status-window-control",
  ] {
    assert!(bar.texts(id).is_none(), "{id} does not show while all is well");
  }
  let (edge, dot) = (bar.bounds("status-bar"), bar.bounds("status-health"));
  assert!(
    (edge.x + edge.width) - (dot.x + dot.width) <= 8.5,
    "the dot ends the bar"
  );
}

#[test]
fn status_health_card_holds_the_runner_agents_queue_server_mcp_and_version() {
  let servers = ServerFacts::One {
    id: orchester_core::id::ServerId::generate(),
    name: "acme-team".to_owned(),
    state: orchester_app::shell::status_bar::ServerState::Connected,
  };
  let mut bar = Bar::of(BarFacts {
    servers,
    ..facts(Some(kontur(online_with_slots(4), ProjectTasks::default())))
  });
  assert!(bar.texts("status-server").is_none(), "connected is quiet");
  let card = bar
    .card("status-health", "status-health-card")
    .expect("the card shows on hover");
  for words in [
    "Runner online",
    "Agents",
    "0 of 4 at work",
    "Queued",
    "0 tasks",
    "Team server",
    "acme-team · connected",
    "MCP server",
    "127.0.0.1:52341",
    "Orchester",
    &format!("v{}", env!("CARGO_PKG_VERSION")),
    "Press to open the runner log. Tokens are on the usage card.",
  ] {
    assert!(card.iter().any(|text| text == words), "{words} in {card:?}");
  }
  assert!(
    card.iter().any(|text| text.starts_with("since ")),
    "since when: {card:?}"
  );
  bar.press("status-health");
  assert!(bar.asked().contains(&BarAction::OpenRunnerLog));
}

#[test]
fn status_health_card_without_a_project_holds_the_apps_own() {
  let mut bar = Bar::of(facts(None));
  assert!(bar.texts("status-runner").is_none());
  assert_eq!(
    bar.texts("status-window-control"),
    Some(vec!["Agents can control this window".to_owned()])
  );
  let card = bar
    .card("status-health", "status-health-card")
    .expect("the card shows on hover");
  assert!(!card.iter().any(|text| text == "Runner online"), "{card:?}");
  assert!(card.iter().any(|text| text == "127.0.0.1:52341"), "{card:?}");
}

#[test]
fn status_runner_says_each_state_but_online_and_the_dot_leaves() {
  let since = SystemTime::now() - Duration::from_secs(60);
  let time = orchester_workspace::display::clock_time(since);
  let failed = |cause, attempts| RunnerFacts {
    trouble_since: Some(since),
    ..runner(RunnerState::Failed { cause, attempts })
  };
  let reconnecting = RunnerFacts {
    next_attempt: Some(Instant::now() + Duration::from_millis(7_500)),
    ..runner(RunnerState::Reconnecting {
      attempt: 2,
      cause: RunnerFailure::Lost,
    })
  };
  let cases = [
    (runner(RunnerState::Starting), vec!["Runner starting".to_owned()]),
    (reconnecting, vec!["Reconnecting to runner".to_owned()]),
    (runner(RunnerState::Stopping), vec!["Stopping runner…".to_owned()]),
    (runner(RunnerState::Stopped), vec!["Runner stopped".to_owned()]),
    (
      failed(RunnerFailure::AlreadyServed, 0),
      vec![
        "Runner offline".to_owned(),
        "· another Orchester app has this project open".to_owned(),
      ],
    ),
    (
      failed(
        RunnerFailure::MissingExecutable {
          path: PathBuf::from("orchester-runner"),
        },
        0,
      ),
      vec![
        "Runner offline".to_owned(),
        "· the runner program is missing".to_owned(),
      ],
    ),
    (
      failed(RunnerFailure::Lost, 5),
      vec!["Runner offline".to_owned(), format!("· 5 attempts since {time}")],
    ),
    (
      failed(RunnerFailure::StartFailed, 0),
      vec!["Runner offline".to_owned(), "· did not start".to_owned()],
    ),
  ];
  for (runner, words) in cases {
    let state = runner.state.clone();
    let reconnecting = matches!(state, RunnerState::Reconnecting { .. });
    let mut bar = Bar::of(facts(Some(kontur(runner, ProjectTasks::default()))));
    assert_eq!(bar.texts("status-runner"), Some(words), "{state:?}");
    assert!(bar.texts("status-health").is_none(), "{state:?}: the dot leaves");
    assert_eq!(
      bar.texts("status-runner-retry"),
      reconnecting.then(|| vec!["· attempt 2 of 5 · next in 8 s".to_owned()]),
      "the countdown is one of the bar's times"
    );
    let restarts = bar.texts("status-restart-runner").is_some();
    assert_eq!(restarts, state.can_restart(), "{state:?}: Restart runner");
    assert_eq!(bar.texts("status-open-runner-log").is_some(), restarts, "{state:?}");
  }
  let mut online = Bar::of(facts(Some(kontur(online_with_slots(4), ProjectTasks::default()))));
  assert!(online.texts("status-runner").is_none(), "online, the dot stands for it");
}

/// `20 · Runner offline · hover` (`nyZ38`): since when, how often it was tried, when next, and the runs waiting.
#[test]
fn status_runner_card_says_tries_the_next_try_and_runs_waiting() {
  let offline = RunnerFacts {
    trouble_since: Some(SystemTime::now()),
    interrupted: 2,
    ..runner(RunnerState::Failed {
      cause: RunnerFailure::Lost,
      attempts: 5,
    })
  };
  let mut bar = Bar::of(facts(Some(kontur(offline, ProjectTasks::default()))));
  let card = bar
    .card("status-runner", "status-runner-card")
    .expect("the card shows on hover");
  for words in ["Runner offline", "Tried", "5 times", "Runs waiting", "2 runs lost their agents"] {
    assert!(card.iter().any(|text| text == words), "{words} in {card:?}");
  }
  let reconnecting = RunnerFacts {
    next_attempt: Some(Instant::now() + Duration::from_millis(7_500)),
    ..runner(RunnerState::Reconnecting {
      attempt: 2,
      cause: RunnerFailure::Lost,
    })
  };
  let mut bar = Bar::of(facts(Some(kontur(reconnecting, ProjectTasks::default()))));
  let card = bar
    .card("status-runner", "status-runner-card")
    .expect("the card shows on hover");
  assert!(card.iter().any(|text| text == "Next try"), "{card:?}");
  let mut starting = Bar::of(facts(Some(kontur(runner(RunnerState::Starting), ProjectTasks::default()))));
  assert!(
    starting.card("status-runner", "status-runner-card").is_none(),
    "a starting runner has no card"
  );
}

#[test]
fn status_runner_actions_restart_and_open_the_log() {
  let offline = runner(RunnerState::Failed {
    cause: RunnerFailure::AlreadyServed,
    attempts: 0,
  });
  let mut bar = Bar::of(facts(Some(kontur(offline, ProjectTasks::default()))));
  bar.press("status-restart-runner");
  bar.press("status-open-runner-log");
  bar.press("status-runner");
  assert_eq!(
    bar.asked(),
    [
      BarAction::RestartRunner,
      BarAction::OpenRunnerLog,
      BarAction::OpenRunnerLog
    ]
  );
}

/// Waiting requests show in the title bar's *Needs you*; the bar keeps only the runs that auto-approve, with *Stop*.
#[test]
fn status_stop_ends_auto_approval_and_waiting_requests_stay_off_the_bar() {
  let auto = auto_approving();
  let tasks = ProjectTasks {
    pending_tool_approvals: Arc::new(vec![approval(), approval()]),
    auto_approving: Arc::new(vec![auto.clone()]),
    ..ProjectTasks::default()
  };
  let mut bar = Bar::of(facts(Some(kontur(runner(RunnerState::Online), tasks))));
  assert!(bar.texts("status-approvals").is_none());
  assert_eq!(
    bar.texts("status-auto-approving-run"),
    Some(vec!["· Run 142 · KON-42".to_owned()])
  );
  bar.press("status-auto-approving-stop");
  assert_eq!(bar.asked(), [BarAction::StopAutoApproving(vec![auto.run])]);
}
