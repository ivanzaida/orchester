//! The cases read off the sources beyond the first fixtures: merges by a person, outside git or the Orchestrator's
//! chat, an agent's question and a question with answers, each decision, each limit, a group's budget and a reached
//! plan limit.

use super::*;

/// A run of a new task titled `title`, started at `now`.
fn started(fixture: &mut Fixture, title: &str, now: i64) -> Run {
  let task = fixture.task(title, at(now));
  fixture.started_run(task, at(now + 1))
}

fn first_execution(run: &Run) -> orchester_core::id::StepExecutionId {
  run.current_attempt().expect("attempt").executions()[0].id()
}

#[test]
fn notifications_read_a_merge_by_a_person_or_outside_but_not_the_orchestrators() {
  let mut fixture = Fixture::new();
  let mut merged = started(&mut fixture, "Add sign-out", 10);
  fixture.succeed(&mut merged, at(20));
  let receipt = RunMerge {
    run: merged.id(),
    into: BranchName::new("main").expect("a branch"),
    revision: Revision::new(&"b".repeat(40)).expect("a revision"),
    kind: MergeKind::FastForward,
    merged_at: at(30),
  };
  fixture.db.save_run_merge(&receipt).expect("record the merge");
  let tasks = load(&fixture);
  let project = project();

  let events = events_of(&project, &tasks);
  assert_eq!(kinds(&events), [&Kind::Merged]);
  assert_eq!(events[0].group(), NotificationGroup::Finished);
  assert!(matches!(
    events[0].target,
    Target::Task {
      page: TaskPage::Summary,
      ..
    }
  ));

  let chat_merges = [merged.id()];
  let reading = ProjectReading {
    tasks: &tasks,
    runner: None,
    merges: &[],
    chat_merges: &chat_merges,
  };
  assert!(
    sources::of_project(&project, &reading).is_empty(),
    "a merge the chat carried out is told in the chat"
  );
}

#[test]
fn notifications_take_only_a_merge_the_chat_carried_out_itself_for_the_orchestrators() {
  let mut fixture = Fixture::new();
  let run = started(&mut fixture, "Add sign-out", 10);
  let merge = |outcome: ActionOutcome, change: Change| ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "Add sign-out".to_owned(),
      change,
    })),
    ..ChatAction::new("merge_run", Some("ORC-1".to_owned()), outcome)
  };
  let merging = Change::Merge {
    run: run.id(),
    number: 1,
    checkpoint: None,
    into: None,
  };
  assert_eq!(
    merged_by_chat(&merge(ActionOutcome::Done, merging.clone())),
    Some(run.id())
  );
  assert_eq!(
    merged_by_chat(&merge(ActionOutcome::Applied, merging.clone())),
    None,
    "a merge a person applied is theirs"
  );
  assert_eq!(merged_by_chat(&merge(ActionOutcome::Waits, merging)), None);
  let cancel = Change::Cancel {
    run: run.id(),
    number: 1,
  };
  assert_eq!(merged_by_chat(&merge(ActionOutcome::Done, cancel)), None);
}

#[test]
fn notifications_read_an_agents_question_and_a_question_with_answers_without_their_text() {
  let mut fixture = Fixture::new();
  let mut asking = started(&mut fixture, "Add login", 10);
  let execution = first_execution(&asking);
  let invocation = fixture
    .db
    .start_invocation(asking.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at(21))
    .expect("invocation")
    .id;
  let events = asking.wait_step(execution, WaitReason::Message, at(22)).expect("waits");
  fixture.record(&asking, &events);
  let question = Message::from_agent(
    asking.task(),
    asking.id(),
    (execution, invocation),
    AgentMessage::Question,
    MessageBody::new("Keep email sign-in?").expect("a message"),
    at(22),
  );
  fixture.db.add_agent_message(&question).expect("stored");

  let choosing = started(&mut fixture, "Place the docs button", 30);
  let execution = first_execution(&choosing);
  let invocation = fixture
    .db
    .start_invocation(
      choosing.id(),
      execution,
      &ModelId::new("gpt-5.5").expect("model"),
      at(41),
    )
    .expect("invocation")
    .id;
  let options = vec![
    QuestionOption::new("Top bar", None),
    QuestionOption::new("Sidebar", None),
  ];
  let asked = QuestionRequest::new(
    "ask-1".to_owned(),
    (choosing.id(), execution, invocation),
    "Designer".to_owned(),
    vec![Question::new("Where should the docs button go?", None, options, false).expect("a question")],
    (at(42), QuestionTurn::InTurn),
  )
  .expect("a request");
  fixture.db.ask_questions(&asked).expect("the question waits");

  let project = project();
  let events = events_of(&project, &load(&fixture));
  assert_eq!(
    kinds(&events),
    [
      &Kind::Question {
        agent: "Builder".into()
      },
      &Kind::Question {
        agent: "Designer".into()
      },
    ]
  );
  assert!(events.iter().all(|event| event.group() == NotificationGroup::NeedsYou));
  for event in &events {
    assert!(
      matches!(
        event.target,
        Target::Task {
          page: TaskPage::Conversation,
          ..
        }
      ),
      "a question opens its card in the conversation: {:?}",
      event.target
    );
  }
  assert_ne!(events[0].key, events[1].key);

  let mut notifier = Notifier::default();
  notifier.observe(Scope::Project(project.id), Vec::new(), at(0));
  notifier.observe(Scope::Project(project.id), events, at(1));
  let shown = notifier.due(&view(at(1), false, &NotificationSettings::default()));
  let words = format!("{shown:?}");
  for text in ["Keep email", "docs button go", "Top bar"] {
    assert!(!words.contains(text), "{text} is not shown: {words}");
  }
}

#[test]
fn notifications_read_each_decision_a_run_waits_for() {
  let mut fixture = Fixture::new();
  let mut unchanged = started(&mut fixture, "Check again", 10);
  let execution = first_execution(&unchanged);
  let events = unchanged
    .fail_step(execution, StepFailure::Unchanged, at(20))
    .expect("waits for the person");
  fixture.record(&unchanged, &events);

  let mut broke = started(&mut fixture, "Break twice", 30);
  let execution = first_execution(&broke);
  let events = broke
    .fail_step(execution, StepFailure::AgentFailed, at(40))
    .expect("restarts");
  fixture.record(&broke, &events);
  let execution = first_execution(&broke);
  let events = broke.start_step(execution, at(41)).expect("starts again");
  fixture.record(&broke, &events);
  let events = broke
    .fail_step(execution, StepFailure::AgentFailed, at(42))
    .expect("waits for the person");
  fixture.record(&broke, &events);

  let events = events_of(&project(), &load(&fixture));
  assert_eq!(
    kinds(&events),
    [
      &Kind::Decision(DecisionWhy::Unchanged),
      &Kind::Decision(DecisionWhy::BrokeAgain),
    ]
  );
  for event in &events {
    assert_eq!(event.group(), NotificationGroup::NeedsYou);
    assert!(
      matches!(
        event.target,
        Target::Task {
          page: TaskPage::Summary,
          card: Some(SummaryCard::Retries),
          ..
        }
      ),
      "a decision opens its card on the Summary: {:?}",
      event.target
    );
  }
}

#[test]
fn notifications_read_each_limit_a_run_stops_at() {
  let mut fixture = Fixture::new();
  let mut stop = |title: &str, now: i64, limit: fn(&Run) -> RunLimit| {
    let mut run = started(&mut fixture, title, now);
    let events = run.stop_at_limit(limit(&run), at(now + 2)).expect("stopped");
    fixture.record(&run, &events);
  };
  stop("Time", 10, |_| RunLimit::Time);
  stop("Step", 20, |run| RunLimit::StepBudget {
    execution: first_execution(run),
  });
  stop("Plan", 30, |run| RunLimit::PlanLimit {
    execution: first_execution(run),
  });
  let events = events_of(&project(), &load(&fixture));
  assert_eq!(
    kinds(&events),
    [
      &Kind::Limit(Some(LimitKind::Time)),
      &Kind::Limit(Some(LimitKind::StepBudget)),
      &Kind::Limit(Some(LimitKind::PlanLimit)),
    ]
  );
  assert!(
    events
      .iter()
      .all(|event| event.group() == NotificationGroup::Stopped && event.batch.is_none()),
    "a run's own limit is no batch"
  );
}

#[test]
fn notifications_batch_the_runs_a_groups_budget_stopped() {
  let mut fixture = Fixture::new();
  for (title, now) in [("One", 10), ("Two", 20)] {
    let mut run = started(&mut fixture, title, now);
    let events = run.stop_at_limit(RunLimit::GroupBudget, at(now + 2)).expect("stopped");
    fixture.record(&run, &events);
  }
  let project = project();
  let events = events_of(&project, &load(&fixture));
  assert_eq!(kinds(&events), [&Kind::Limit(Some(LimitKind::GroupBudget)); 2]);
  assert!(events[0].batch.is_some() && events[0].batch == events[1].batch);
  let mut notifier = Notifier::default();
  notifier.observe(Scope::Project(project.id), Vec::new(), at(0));
  notifier.observe(Scope::Project(project.id), events, at(1));
  let shown = notifier.due(&view(at(1), false, &NotificationSettings::default()));
  assert_eq!(shown.len(), 1, "one notification for the batch");
  assert_eq!(shown[0].title.key, "notifications.title.limit.groupMany");
  assert_eq!(shown[0].title.args, [("count", "2".to_owned())]);
}

#[test]
fn notifications_read_a_plan_limit_reached() {
  let name = |_: &LimitReading| -> Arc<str> { "Work".into() };
  let window = LimitWindow::new(300, None, Some(at(10_000)), LimitState::Reached, false).expect("window");
  let mut reached = reading(0.0, 10_000);
  reached.limits.windows = vec![window];
  let mut readings = PlanReadings::default();
  readings.observe(reached);
  let events = sources::of_plan_limits(&readings, at(2_000), name);
  assert_eq!(kinds(&events), [&Kind::PlanLimitNear { percent: 100 }]);
  assert_eq!(events[0].group(), NotificationGroup::Finished);
}
