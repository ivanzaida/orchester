//! The activity slot, the tasks that could not be read, *Stop* and the *Background operations* popover.

use super::*;

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
    Some("Saving draft · Quick fix…"),
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
fn status_unread_tasks_take_the_slot_with_try_again() {
  let mut log = ActivityLog::default();
  log.start(operation("Feature delivery"), Instant::now());
  let mut first = Bar::of(BarFacts {
    log: log.clone(),
    ..facts(Some(ProjectFacts {
      tasks: None,
      read_failure: Some(ReadFailure::First(Message::new("errors.runnerLost"))),
      ..kontur(runner(RunnerState::Online), ProjectTasks::default())
    }))
  });
  let text = first.texts("status-activity-text").expect("the slot shows").join("");
  assert!(text.starts_with("Tasks couldn't be read · "), "{text}");
  let stale = |runner| ProjectFacts {
    read_failure: Some(ReadFailure::Again(Message::new("errors.runnerLost"))),
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

#[test]
fn status_recovery_stays_pinned_with_review_until_the_person_decides() {
  let mut log = ActivityLog::default();
  log.record(
    Some("Kontur".into()),
    Message::new("statusBar.activity.runnerReconnected"),
    Target::Nothing,
    SystemTime::now(),
  );
  let recovering = |count| ProjectFacts {
    undecided_runs: count,
    ..kontur(runner(RunnerState::Online), ProjectTasks::default())
  };
  let mut bar = Bar::of(BarFacts {
    log: log.clone(),
    ..facts(Some(recovering(2)))
  });
  assert_eq!(
    bar.texts("status-recovery-text").map(|texts| texts.join("")).as_deref(),
    Some("2 runs interrupted when Orchester closed")
  );
  assert!(
    bar.texts("status-activity-text").is_none(),
    "it takes the activity slot"
  );
  bar.press("status-recovery-review");
  assert_eq!(
    bar.asked(),
    [
      BarAction::ReviewInterrupted(project_id()),
      BarAction::Go(Route::Project(
        project_id(),
        ProjectPage::Overview(OverviewTab::default())
      )),
    ]
  );
  let mut one = Bar::of(facts(Some(recovering(1))));
  assert_eq!(
    one.texts("status-recovery-text").map(|texts| texts.join("")).as_deref(),
    Some("1 run interrupted when Orchester closed")
  );
  let mut decided = Bar::of(BarFacts {
    log,
    ..facts(Some(recovering(0)))
  });
  assert!(decided.texts("status-recovery").is_none());
  assert_eq!(
    decided
      .texts("status-activity-text")
      .map(|texts| texts.join(""))
      .as_deref(),
    Some("Runner reconnected"),
    "once decided the slot shows the activity again"
  );
}

#[test]
fn status_stop_waits_while_it_stops_and_says_why_it_could_not() {
  let tasks = ProjectTasks {
    auto_approving: Arc::new(vec![auto_approving()]),
    ..ProjectTasks::default()
  };
  let project = || Some(kontur(runner(RunnerState::Online), tasks.clone()));
  let mut busy = Bar::of(BarFacts {
    stopping: Stopping::Busy,
    ..facts(project())
  });
  assert_eq!(
    busy.texts("status-auto-approving-stop"),
    Some(vec!["Stopping…".to_owned()])
  );
  busy.press("status-auto-approving-stop");
  assert!(busy.asked().is_empty(), "no second stop while one is under way");
  let mut failed = Bar::of(BarFacts {
    stopping: Stopping::Failed(Message::new("errors.runnerLost")),
    ..facts(project())
  });
  let why = failed
    .texts("status-auto-approving-error")
    .expect("why it could not stop")
    .join("");
  assert!(why.starts_with("Couldn't stop auto-approving · "), "{why}");
  failed.press("status-auto-approving-stop");
  assert_eq!(failed.asked().len(), 1, "Stop can be pressed again");
}

#[test]
fn status_several_operations_open_the_background_operations() {
  let mut log = ActivityLog::default();
  log.start(operation("Feature delivery"), Instant::now());
  log.start(operation("Quick fix"), Instant::now());
  let mut bar = Bar::of(BarFacts {
    log,
    ..facts(Some(kontur(runner(RunnerState::Online), ProjectTasks::default())))
  });
  let item = bar.texts("status-activity").expect("the slot shows");
  assert_eq!(item[0], "2 operations");
  assert!(bar.texts("operations-popover").is_none());
  bar.press("status-activity");
  let popover = bar.texts("operations-popover").expect("the popover opens");
  for words in [
    "Background operations",
    "2 running",
    "RUNNING",
    "Saving draft · Quick fix…",
  ] {
    assert!(popover.iter().any(|text| text == words), "{words} in {popover:?}");
  }
}
