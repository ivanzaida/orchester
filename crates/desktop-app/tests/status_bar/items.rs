//! What the runner, the counts and the requests' items say and ask for.

use super::*;

#[test]
fn status_items_without_a_project_are_the_apps_own() {
  let mut bar = Bar::of(facts(None));
  for id in [
    "status-project",
    "status-runner",
    "status-agents",
    "status-needs-you",
    "status-tokens",
  ] {
    assert!(bar.texts(id).is_none(), "{id} shows only with a project");
  }
  assert_eq!(bar.texts("status-mcp"), Some(vec!["MCP 127.0.0.1:52341".to_owned()]));
  assert_eq!(
    bar.texts("status-window-control"),
    Some(vec!["Agents can control this window".to_owned()])
  );
  assert_eq!(
    bar.texts("status-version"),
    Some(vec![format!("v{}", env!("CARGO_PKG_VERSION"))])
  );
  let mut unavailable = Bar::of(BarFacts {
    mcp: McpStatus::unavailable(McpKind::Installation, McpSettings::with_window_control(true)),
    ..facts(None)
  });
  assert_eq!(
    unavailable.texts("status-mcp"),
    Some(vec!["MCP unavailable".to_owned()])
  );
  assert!(unavailable.texts("status-window-control").is_none());
}

/// The project item says the project's name; its folder shows in a tooltip that hangs from the item's
/// start, the glyph included in what the pointer rests on (`z61KIT`).
#[test]
fn status_project_is_its_name_with_its_folder_in_a_tooltip() {
  let mut bar = Bar::of(facts(Some(kontur(
    runner(RunnerState::Online),
    ProjectTasks::default(),
  ))));
  assert_eq!(bar.texts("status-project"), Some(vec!["Kontur".to_owned()]));
  let (item, name) = (bar.bounds("status-project"), bar.bounds("status-project-text"));
  assert_eq!(
    name.x - item.x,
    8.0 + 12.0 + 6.0,
    "padding, glyph and gap before the name"
  );
  assert_eq!(bar.tooltip("status-project-text"), Some(vec![ROOT.to_owned()]));
  assert_eq!(
    bar.bounds("status-project-text-tooltip").x,
    item.x,
    "the tooltip starts at the item"
  );
}

#[test]
fn status_runner_says_each_state() {
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
    (runner(RunnerState::Online), vec!["Runner online".to_owned()]),
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
    assert_eq!(
      bar.texts("status-runner-retry"),
      reconnecting.then(|| vec!["· attempt 2 of 5 · next in 8 s".to_owned()]),
      "the countdown is one of the bar's times"
    );
    let restarts = bar.texts("status-restart-runner").is_some();
    assert_eq!(restarts, state.can_restart(), "{state:?}: Restart runner");
    assert_eq!(bar.texts("status-open-runner-log").is_some(), restarts, "{state:?}");
  }
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

#[test]
fn status_counts_show_zero_and_dash_only_while_unknown() {
  let mut starting = Bar::of(facts(Some(kontur(
    runner(RunnerState::Starting),
    ProjectTasks::default(),
  ))));
  assert_eq!(
    starting.texts("status-counts").map(|texts| texts[..2].to_vec()),
    Some(vec!["Agents".to_owned(), "—".to_owned()])
  );
  assert_eq!(
    starting.texts("status-counts").map(|texts| texts[2..4].to_vec()),
    Some(vec!["Queued".to_owned(), "0".to_owned()])
  );
  assert_eq!(
    starting.texts("status-counts").map(|texts| texts[4..].to_vec()),
    Some(vec!["0".to_owned(), "need you".to_owned()]),
    "0 stays visible"
  );
  assert_eq!(
    starting.texts("status-tokens"),
    Some(vec!["3.8M / 20M today".to_owned()])
  );
  let online = RunnerFacts {
    work: Some(WorkSummary {
      agent_slots: NonZeroU32::new(4).expect("slots"),
      runs: Vec::new(),
    }),
    ..runner(RunnerState::Online)
  };
  let tasks = ProjectTasks {
    pending_tool_approvals: Arc::new(vec![approval()]),
    ..ProjectTasks::default()
  };
  let mut bar = Bar::of(facts(Some(kontur(online, tasks))));
  assert_eq!(bar.texts("status-agents-value"), Some(vec!["0/4".to_owned()]));
  assert_eq!(
    bar.texts("status-counts").map(|texts| texts[4..].to_vec()),
    Some(vec!["1".to_owned(), "needs you".to_owned()])
  );
  let offline = runner(RunnerState::Failed {
    cause: RunnerFailure::Lost,
    attempts: 5,
  });
  let mut down = Bar::of(facts(Some(kontur(offline, ProjectTasks::default()))));
  assert_eq!(
    down.texts("status-agents-value"),
    Some(vec!["—".to_owned()]),
    "unknown unless the runner is online and reported"
  );
  let mut reading = Bar::of(facts(Some(ProjectFacts {
    tasks: None,
    usage: None,
    ..kontur(runner(RunnerState::Online), ProjectTasks::default())
  })));
  assert_eq!(
    reading.texts("status-counts").map(|texts| texts[4..].to_vec()),
    Some(vec!["—".to_owned(), "need you".to_owned()]),
    "a dash while the tasks are read, so nothing moves once they are"
  );
  assert_eq!(reading.texts("status-queued-value"), Some(vec!["—".to_owned()]));
  assert_eq!(reading.texts("status-tokens"), Some(vec!["— / — today".to_owned()]));
  assert_eq!(reading.texts("status-agents-value"), Some(vec!["—".to_owned()]));
}

#[test]
fn status_counts_open_tasks_and_the_overview() {
  let mut bar = Bar::of(facts(Some(kontur(
    runner(RunnerState::Online),
    ProjectTasks::default(),
  ))));
  bar.press("status-needs-you");
  bar.press("status-queued");
  let at = |page| BarAction::Go(Route::Project(project_id(), page));
  assert_eq!(
    bar.asked(),
    [
      at(ProjectPage::Overview(OverviewTab::NeedsYou)),
      at(ProjectPage::Overview(OverviewTab::Queued)),
    ]
  );
}

#[test]
fn status_review_opens_the_requests_conversation_and_stop_ends_auto_approval() {
  let auto = auto_approving();
  let tasks = ProjectTasks {
    pending_tool_approvals: Arc::new(vec![approval(), approval()]),
    auto_approving: Arc::new(vec![auto.clone()]),
    ..ProjectTasks::default()
  };
  let mut bar = Bar::of(facts(Some(kontur(runner(RunnerState::Online), tasks))));
  assert_eq!(
    bar.texts("status-approvals"),
    Some(vec!["2".to_owned(), "approvals waiting".to_owned()])
  );
  assert_eq!(
    bar.texts("status-approvals-asks"),
    Some(vec!["· Builder asks to run cargo add serde · KON-42".to_owned()])
  );
  assert_eq!(
    bar.texts("status-auto-approving-run"),
    Some(vec!["· Run 142 · KON-42".to_owned()])
  );
  bar.press("status-approvals-review");
  bar.press("status-auto-approving-stop");
  let key = "KON-42".parse().expect("a task key");
  assert_eq!(
    bar.asked(),
    [
      BarAction::Go(Route::Project(
        project_id(),
        ProjectPage::Task(key, TaskView::Conversation)
      )),
      BarAction::StopAutoApproving(vec![auto.run]),
    ]
  );
}
