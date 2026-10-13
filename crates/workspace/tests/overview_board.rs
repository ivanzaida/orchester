//! The new Overview as data (`tasks::board`, `tasks::attention`, `tasks::ready_to_run`): what waits for
//! the person, the oldest first, interrupted runs among it, and tool requests whose run cannot be read; the
//! tabs and their tasks; the figures; the notices; and what an empty project is ready to run with.

mod support;

use std::num::NonZeroU32;

use orchester_core::{
  connection::ConnectionStatus,
  resource::{ModelId, VersionNumber},
  run::{PendingToolApproval, WaitReason},
  store::{ActivityStore, RunStore},
};
use orchester_workspace::tasks::{
  attention::{self, NeedKind, Waited},
  board::{self, BoardTab, Figures},
  data::ProjectTasks,
  ready_to_run::{CheckKind, ProjectReadiness},
};

/// A completed task, a running one, one whose run waits for an answer and a draft.
fn project() -> (support::Fixture, ProjectTasks) {
  let mut fixture = support::Fixture::keyed("KON");
  let done = fixture.task("Add login", support::at(10));
  let mut run = fixture.started_run(done, support::at(20));
  fixture.succeed(&mut run, support::at(30));
  let running = fixture.task("Fix zoom", support::at(40));
  fixture.started_run(running, support::at(50));
  let asking = fixture.task("Write docs", support::at(60));
  let mut run = fixture.started_run(asking, support::at(70));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run
    .wait_step(execution, WaitReason::Message, support::at(80))
    .expect("the step waits for a message");
  fixture.db.append_events(&run, &events).expect("append");
  fixture.task("Plan release", support::at(90));
  let tasks = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");
  (fixture, tasks)
}

#[test]
fn board_needs_you_lists_what_waits_oldest_first() {
  let (_fixture, tasks) = project();
  let items = attention::items(&tasks);
  let kinds: Vec<NeedKind> = items.iter().map(|item| item.kind).collect();
  assert_eq!(kinds, [NeedKind::ReadyToMerge, NeedKind::Input]);
  assert_eq!(items[0].task.as_ref().map(|task| &*task.key), Some("KON-1"));
  assert_eq!(items[0].context().args[1].1, "Add login");
  assert_eq!(attention::count(&tasks), 2);
  assert_eq!(BoardTab::NeedsYou.count(&tasks), 2);
  assert_eq!(
    items[1].waited(support::at(80 + 41 * 60_000)),
    Waited::For(orchester_workspace::messages::Message::new("time.minutes").with("count", "41".to_owned()))
  );
}

#[test]
fn board_needs_you_keeps_a_tool_request_whose_run_cannot_be_read() {
  let mut fixture = support::Fixture::keyed("KON");
  let task = fixture.task("Add login", support::at(10));
  let run = fixture.started_run(task, support::at(20));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let invocation = fixture
    .db
    .start_invocation(
      run.id(),
      execution,
      &ModelId::new("gpt-5.5").expect("model"),
      support::at(21),
    )
    .expect("invocation")
    .id;
  let approval = PendingToolApproval {
    id: format!("{invocation}:call_1"),
    run: run.id(),
    execution,
    invocation,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: String::new(),
    action: "pnpm build".to_owned(),
    target: None,
    requested_at: support::at(22),
    native: None,
  };
  fixture.db.request_tool_approval(&approval).expect("the request waits");
  // The store has no API for a damaged record: an edit outside Orchester leaves it.
  let changed = rusqlite::Connection::open(fixture.repo.path().join(".orchester").join("orchester.db"))
    .expect("the project database")
    .execute(
      "UPDATE runs SET snapshot = 'not json' WHERE id = ?1",
      [run.id().to_string()],
    )
    .expect("damage the run");
  assert_eq!(changed, 1);

  let tasks = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");
  assert_eq!(tasks.pending_tool_approvals[0].task_id, None, "the run cannot be read");
  let items = attention::items(&tasks);
  let [item] = items.as_slice() else {
    panic!("the request alone waits: {items:?}");
  };
  assert_eq!((item.kind, &item.task), (NeedKind::Approval, &None));
  assert_eq!(item.title.args[0].1, "pnpm build");
  assert_eq!(item.context().key, "overview.needs.contextUnread");
  assert_eq!(attention::count(&tasks), 1);
  assert_eq!(Figures::of(&tasks, support::at(0)).needs_you, 1);
}

#[test]
fn board_counts_interrupted_runs_as_needing_the_person_and_as_running() {
  let (_fixture, tasks) = project();
  let away = tasks.without_runner();
  let items = attention::items(&away);
  assert!(items.iter().any(|item| {
    item.kind == NeedKind::Interrupted && item.task.as_ref().is_some_and(|task| &*task.key == "KON-2")
  }));
  assert!(matches!(
    items
      .iter()
      .find(|item| item.kind == NeedKind::Interrupted)
      .map(|item| item.waited(support::at(0))),
    Some(Waited::Since(_))
  ));
  assert_eq!(
    BoardTab::Running.count(&away),
    1,
    "the interrupted run stays where it was at work"
  );
  assert_eq!(BoardTab::NeedsYou.count(&away), 3);
  let figures = Figures::of(&away, support::at(0));
  assert_eq!((figures.running, figures.interrupted, figures.needs_you), (0, 1, 3));
  let running = figures.running_now(false, None);
  assert_eq!(running.count, Some(1));
  assert!(running.interrupted);
  assert_eq!(running.detail.key, "overview.metrics.interrupted.one");
}

#[test]
fn board_running_now_is_unknown_without_a_runner_and_says_when_nothing_ran() {
  let (_fixture, tasks) = project();
  let figures = Figures::of(&tasks, support::at(0));
  assert_eq!(figures.running, 1);
  let offline = figures.running_now(false, None);
  assert_eq!(
    (offline.count, offline.detail.key),
    (None, "overview.metrics.agentsUnknown")
  );
  assert_eq!(figures.running_now(true, None).count, Some(1));
  let empty = Figures::default().running_now(true, None);
  assert_eq!(empty.detail.key, "overview.board.noRuns");
}

#[test]
fn board_tabs_hold_their_tasks_newest_first() {
  let (_fixture, tasks) = project();
  let keys = |tab: BoardTab| -> Vec<String> {
    tab
      .rows(&tasks, 6, ", ")
      .iter()
      .map(|row| row.key.to_string())
      .collect()
  };
  assert_eq!(keys(BoardTab::Running), ["KON-2"]);
  assert_eq!(keys(BoardTab::NeedsYou), ["KON-3", "KON-1"]);
  assert!(keys(BoardTab::Queued).is_empty());
  assert!(keys(BoardTab::Failed).is_empty());
  assert_eq!(BoardTab::NeedsYou.rows(&tasks, 1, ", ").len(), 1);
}

#[test]
fn board_needs_you_rows_say_what_waits_as_their_items_do() {
  let (_fixture, tasks) = project();
  let items = attention::items(&tasks);
  for row in BoardTab::NeedsYou.rows(&tasks, 6, ", ") {
    let item = items
      .iter()
      .find(|item| item.task.as_ref().is_some_and(|task| task.id == row.id))
      .expect("each waiting task is an item");
    assert_eq!(row.detail, item.title, "{}", row.key);
  }
  let running = &BoardTab::Running.rows(&tasks, 6, ", ")[0];
  assert_ne!(
    running.detail.key, "overview.needs.title.input",
    "other tabs keep their detail"
  );
}

#[test]
fn ready_to_run_reads_the_published_agents_and_pipelines_and_wants_a_connection() {
  let (fixture, _tasks) = project();
  let readiness = ProjectReadiness::load_in(fixture.repo.path()).expect("read the readiness");
  let checks = readiness.checks(", ");
  let kinds: Vec<CheckKind> = checks.iter().map(|check| check.kind).collect();
  assert_eq!(kinds, CheckKind::ALL);
  assert!(checks[0].ready && checks[1].ready);
  assert_eq!(checks[0].detail.args, [("agents", "Builder".to_owned())]);
  assert_eq!(checks[1].detail.key, "overview.ready.pipeline.one");
  assert_eq!(checks[1].detail.args[0].1, "Delivery");
  assert!(!checks[2].ready);
  assert_eq!(checks[2].detail.key, "overview.ready.connection.none");
}

#[test]
fn ready_to_run_wants_a_signed_in_connection_and_says_why_one_is_not() {
  let version = |number| VersionNumber::new(NonZeroU32::new(number).expect("nonzero"));
  let mut readiness = ProjectReadiness {
    agents: Vec::new(),
    pipelines: vec![("Delivery".into(), version(3)), ("Hotfix".into(), version(1))],
    connections: vec![("Codex".into(), ConnectionStatus::SignInRequired)],
  };
  let checks = readiness.checks(", ");
  assert!(!checks[0].ready);
  assert_eq!(checks[0].detail.key, "overview.ready.agents.none");
  assert_eq!(checks[1].detail.key, "overview.ready.pipeline.several");
  assert!(!checks[2].ready);
  assert_eq!(checks[2].detail.key, "overview.ready.connection.signInRequired");
  assert_eq!(checks[2].detail.args, [("connection", "Codex".to_owned())]);
  readiness
    .connections
    .push(("Claude".into(), ConnectionStatus::Connected));
  let connection = &readiness.checks(", ")[2];
  assert!(connection.ready);
  assert_eq!(connection.detail.args, [("connections", "Claude".to_owned())]);
}

#[test]
fn board_tokens_today_say_when_usage_is_not_read() {
  let (figure, detail) = board::tokens_today(None, "Kontur");
  assert_eq!((figure, detail.key), (None, "overview.board.usageUnread"));
}

#[test]
fn board_has_no_notices_without_blocked_or_unreadable_tasks() {
  let (_fixture, tasks) = project();
  assert!(board::notices(&tasks, ", ").is_empty());
}
