//! The status bar as its facts say (`shell::status_bar::StatusBarView`), headless at the design's 1440 ×
//! 900 and at 960: what each item says in each state, what its actions ask the window to do, which of the
//! activity's candidates the slot shows, and the order in which the items give way on a narrow window.

use std::{
  num::{NonZeroU16, NonZeroU32},
  path::PathBuf,
  sync::Arc,
  time::{Duration, Instant, SystemTime},
};

use lurq::{
  app::{App, Tree, events::MouseButton},
  node::element::ElementRef,
};
use orchester_app::{
  i18n,
  mcp::McpStatus,
  routes::{OverviewTab, ProjectPage, Route, TaskView},
  shell::status_bar::{BarAction, BarFacts, BarProps, ProjectFacts, RunnerFacts, StatusBarView},
  theme::{self, Mode},
};
use orchester_core::{
  id::{InvocationId, ProjectId, RunId, StepExecutionId, TaskId},
  limits::ProjectLimits,
  run::{PendingToolApproval, RunNumber, SimilarUnavailable},
  time::Timestamp,
  tokens::TokensUsed,
};
use orchester_ipc::WorkSummary;
use orchester_workspace::{
  activity::{ActivityLog, OperationSpec, Target},
  messages::Message,
  runner::state::{RunnerFailure, RunnerState},
  tasks::{
    approvals::{AlwaysBlocked, ApprovalChoices, ShownApproval},
    data::{AutoApprovingRun, ProjectTasks},
    usage::ProjectUsage,
  },
};
use parking_lot::Mutex;

const ROOT: &str = "H:/projects/kontur";

/// The bar mounted alone, and what its actions asked for.
struct Bar {
  app: App,
  tree: Tree,
  asked: Arc<Mutex<Vec<BarAction>>>,
}

impl Bar {
  fn at(width: u32, facts: BarFacts) -> Self {
    let mut app = App::new();
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(width, 900);
    let asked = Arc::new(Mutex::new(Vec::new()));
    let record = asked.clone();
    let dispatch = Arc::new(move |action| record.lock().push(action));
    tree.mount_root::<StatusBarView>(&mut app, BarProps { facts, dispatch });
    tree.pass_headless(&mut app);
    Self { app, tree, asked }
  }

  fn of(facts: BarFacts) -> Self {
    Self::at(1440, facts)
  }

  fn texts(&mut self, id: &str) -> Option<Vec<String>> {
    let element = self.tree.get_element_by_id(id)?;
    let mut all = Vec::new();
    collect(element, &mut all);
    all.retain(|text| !text.chars().all(|glyph| ('\u{e000}'..='\u{f8ff}').contains(&glyph)));
    Some(all)
  }

  /// The width the element `id` takes; 0 once it gave way.
  fn width(&mut self, id: &str) -> f32 {
    self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
      .width
  }

  fn press(&mut self, id: &str) {
    let bounds = self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    self.tree.mouse_move(x, y);
    self.tree.mouse_down(x, y, MouseButton::Left);
    self.tree.mouse_up(x, y, MouseButton::Left);
    self.tree.pass_headless(&mut self.app);
  }

  fn asked(&self) -> Vec<BarAction> {
    self.asked.lock().clone()
  }
}

fn collect(element: ElementRef<'_>, into: &mut Vec<String>) {
  into.extend(element.text_content().map(str::to_owned));
  for child in element.children() {
    collect(child, into);
  }
}

fn project_id() -> ProjectId {
  "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b".parse().expect("a project id")
}

fn listening() -> McpStatus {
  McpStatus::Listening {
    port: NonZeroU16::new(52341).expect("a port"),
    window_control: true,
  }
}

fn runner(state: RunnerState) -> RunnerFacts {
  RunnerFacts {
    state,
    ..RunnerFacts::starting()
  }
}

/// Kontur with `runner`, its tasks read as `tasks` and today's usage read.
fn kontur(runner: RunnerFacts, tasks: ProjectTasks) -> ProjectFacts {
  ProjectFacts {
    id: project_id(),
    name: "Kontur".to_owned(),
    repo_root: PathBuf::from(ROOT),
    runner,
    tasks: Some(tasks),
    usage: Some(ProjectUsage {
      limits: ProjectLimits::default(),
      today: TokensUsed::new(3_800_000),
    }),
    stale: None,
  }
}

fn facts(project: Option<ProjectFacts>) -> BarFacts {
  BarFacts {
    project,
    log: ActivityLog::default(),
    mcp: listening(),
  }
}

/// A request of Builder in KON-42 to run `cargo add serde`, waiting since now.
fn approval() -> ShownApproval {
  ShownApproval {
    approval: PendingToolApproval {
      id: "request-1".to_owned(),
      run: RunId::generate(),
      execution: StepExecutionId::generate(),
      invocation: InvocationId::generate(),
      agent: "Builder".to_owned(),
      command: true,
      scope: None,
      server: String::new(),
      action: "cargo add serde".to_owned(),
      target: None,
      requested_at: Timestamp::now(),
      native: None,
    },
    provider: None,
    vendor: None,
    model: None,
    read_only: false,
    run: None,
    task: Some("KON-42".into()),
    task_id: Some(TaskId::generate()),
    step: None,
    step_key: None,
    cwd: None,
    choices: ApprovalChoices {
      similar: Err(SimilarUnavailable::Unknown),
      always: Err(AlwaysBlocked::Missing),
      told: "Denied.".into(),
      codex: false,
      category: None,
      secrets: false,
      reason: None,
      auto_asks: true,
    },
    asking: None,
    auto_approving: false,
    review: None,
  }
}

fn auto_approving() -> AutoApprovingRun {
  AutoApprovingRun {
    run: RunId::generate(),
    number: RunNumber::new(NonZeroU32::new(142).expect("a run number")),
    task: TaskId::generate(),
    key: "KON-42".into(),
  }
}

fn operation(name: &str) -> OperationSpec {
  let message = |key: &'static str| Message::new(key).with("name", name.to_owned());
  OperationSpec::new(
    message("statusBar.operations.pipelineDraft.running"),
    message("statusBar.operations.pipelineDraft.done"),
    message("statusBar.operations.pipelineDraft.failed"),
  )
}

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
  assert_eq!(bar.texts("status-mcp"), Some(vec!["MCP on :52341".to_owned()]));
  assert_eq!(
    bar.texts("status-window-control"),
    Some(vec!["Agents can control this window".to_owned()])
  );
  assert_eq!(
    bar.texts("status-version"),
    Some(vec![format!("v{}", env!("CARGO_PKG_VERSION"))])
  );
  let mut unavailable = Bar::of(BarFacts {
    mcp: McpStatus::Unavailable,
    ..facts(None)
  });
  assert_eq!(
    unavailable.texts("status-mcp"),
    Some(vec!["MCP unavailable".to_owned()])
  );
  assert!(unavailable.texts("status-window-control").is_none());
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
    (
      reconnecting,
      vec![
        "Reconnecting to runner".to_owned(),
        "· attempt 2 of 5 · next in 8 s".to_owned(),
      ],
    ),
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
    let mut bar = Bar::of(facts(Some(kontur(runner, ProjectTasks::default()))));
    assert_eq!(bar.texts("status-runner"), Some(words), "{state:?}");
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
    starting.texts("status-agents"),
    Some(vec!["Agents".to_owned(), "—".to_owned()])
  );
  assert_eq!(
    starting.texts("status-queued"),
    Some(vec!["Queued".to_owned(), "0".to_owned()])
  );
  assert_eq!(
    starting.texts("status-needs-you"),
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
    bar.texts("status-needs-you"),
    Some(vec!["1".to_owned(), "needs you".to_owned()])
  );
  let offline = runner(RunnerState::Failed {
    cause: RunnerFailure::Lost,
    attempts: 5,
  });
  let mut down = Bar::of(facts(Some(kontur(offline, ProjectTasks::default()))));
  assert_eq!(
    down.texts("status-agents-value"),
    Some(vec![format!("0/{}", ProjectLimits::default().agent_slots().get())]),
    "no agent works without a runner, of the project's slots"
  );
  let mut reading = Bar::of(facts(Some(ProjectFacts {
    tasks: None,
    usage: None,
    ..kontur(runner(RunnerState::Online), ProjectTasks::default())
  })));
  assert!(reading.texts("status-needs-you").is_none(), "while the tasks are read");
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

#[test]
fn status_activity_slot_keeps_its_priority() {
  let mut log = ActivityLog::default();
  let now = Instant::now();
  log.record(
    Some("Kontur".into()),
    Message::new("statusBar.activity.runnerReconnected"),
    Target::Nothing,
    SystemTime::now(),
  );
  let online = || Some(kontur(runner(RunnerState::Online), ProjectTasks::default()));
  let shown = |log: &ActivityLog, project| {
    let mut bar = Bar::of(BarFacts {
      log: log.clone(),
      ..facts(project)
    });
    bar.texts("status-activity-text").map(|texts| texts.join(""))
  };
  assert_eq!(
    shown(&log, online()).as_deref(),
    Some("Runner reconnected"),
    "the latest event"
  );
  assert_eq!(
    shown(&log, None).as_deref(),
    Some("Kontur · Runner reconnected"),
    "outside a project an event names its project"
  );
  let saving = log.start(operation("Feature delivery"), now);
  log.start(operation("Quick fix"), now);
  assert_eq!(
    shown(&log, online()).as_deref(),
    Some("2 operations · Saving draft · Quick fix…"),
    "operations under way come before the latest event"
  );
  log.fail(saving, Message::new("errors.runnerLost"), SystemTime::now());
  let failure = shown(&log, online()).expect("the failure shows");
  assert!(
    failure.starts_with("Could not save draft · Feature delivery · "),
    "{failure}"
  );
  let offline = Some(kontur(
    runner(RunnerState::Failed {
      cause: RunnerFailure::Lost,
      attempts: 5,
    }),
    ProjectTasks::default(),
  ));
  assert_eq!(shown(&log, offline), None, "a runner problem comes first");
  let starting = Some(kontur(runner(RunnerState::Starting), ProjectTasks::default()));
  assert_eq!(shown(&log, starting), None, "nothing while the runner starts");
}

#[test]
fn status_failure_is_dismissed_from_the_slot() {
  let mut log = ActivityLog::default();
  let id = log.start(operation("Feature delivery"), Instant::now());
  log.fail(id, Message::new("errors.runnerLost"), SystemTime::now());
  let mut bar = Bar::of(BarFacts {
    log,
    ..facts(Some(kontur(runner(RunnerState::Online), ProjectTasks::default())))
  });
  bar.press("status-activity-dismiss");
  assert_eq!(bar.asked(), [BarAction::Resolve(id)]);
}

#[test]
fn status_stale_tasks_take_the_slot_with_try_again() {
  let mut log = ActivityLog::default();
  log.start(operation("Feature delivery"), Instant::now());
  let stale = |runner| ProjectFacts {
    stale: Some(Message::new("errors.runnerLost")),
    ..kontur(runner, ProjectTasks::default())
  };
  let mut bar = Bar::of(BarFacts {
    log: log.clone(),
    ..facts(Some(stale(runner(RunnerState::Online))))
  });
  let text = bar.texts("status-activity-text").expect("the slot shows").join("");
  assert!(text.starts_with("Tasks shown as last read · "), "{text}");
  bar.press("status-read-again");
  assert_eq!(bar.asked(), [BarAction::ReadAgain]);
  let offline = runner(RunnerState::Failed {
    cause: RunnerFailure::Lost,
    attempts: 5,
  });
  let mut down = Bar::of(BarFacts {
    log,
    ..facts(Some(stale(offline)))
  });
  assert!(
    down.texts("status-read-again").is_none(),
    "a runner problem comes first"
  );
}

/// The bar at 960 with everything shown: the version and Tokens today give way, then the counts' words;
/// the hand, the counts, *Review*, *Stop* and MCP stay.
#[test]
fn status_gives_way_in_its_order_at_960() {
  let tasks = ProjectTasks {
    pending_tool_approvals: Arc::new(vec![approval()]),
    auto_approving: Arc::new(vec![auto_approving()]),
    ..ProjectTasks::default()
  };
  let project = || {
    Some(kontur(
      RunnerFacts {
        work: Some(WorkSummary {
          agent_slots: NonZeroU32::new(4).expect("slots"),
          runs: Vec::new(),
        }),
        ..runner(RunnerState::Online)
      },
      tasks.clone(),
    ))
  };
  let mut wide = Bar::of(facts(project()));
  for id in ["status-version", "status-tokens"] {
    assert!(wide.width(id) > 0.0, "{id} shows at 1440");
  }
  let mut narrow = Bar::at(960, facts(project()));
  for id in ["status-version", "status-tokens"] {
    assert_eq!(narrow.width(id), 0.0, "{id} gives way at 960");
  }
  for id in [
    "status-needs-you-value",
    "status-agents-value",
    "status-queued-value",
    "status-approvals-review",
    "status-auto-approving-stop",
    "status-mcp",
  ] {
    assert!(narrow.width(id) > 0.0, "{id} never gives way");
  }
}
