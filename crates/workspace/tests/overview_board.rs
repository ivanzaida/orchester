//! The new Overview as data (`tasks::board`, `tasks::attention`): what waits for the person, the oldest
//! first, interrupted runs among it; the tabs and their tasks; the figures; and the notices.

mod support;

use orchester_core::{run::WaitReason, store::RunStore};
use orchester_workspace::tasks::{
  attention::{self, NeedKind, Waited},
  board::{self, BoardTab, Figures},
  data::ProjectTasks,
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
  assert_eq!(&*items[0].key, "KON-1");
  assert_eq!(items[0].context().args[1].1, "Add login");
  assert_eq!(attention::count(&tasks), 2);
  assert_eq!(BoardTab::NeedsYou.count(&tasks), 2);
  assert_eq!(
    items[1].waited(support::at(80 + 41 * 60_000)),
    Waited::For(orchester_workspace::messages::Message::new("time.minutes").with("count", "41".to_owned()))
  );
}

#[test]
fn board_counts_interrupted_runs_as_needing_the_person_and_as_running() {
  let (_fixture, tasks) = project();
  let away = tasks.without_runner();
  let items = attention::items(&away);
  assert!(
    items
      .iter()
      .any(|item| item.kind == NeedKind::Interrupted && &*item.key == "KON-2")
  );
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
fn board_tokens_today_say_when_usage_is_not_read() {
  let (figure, detail) = board::tokens_today(None, "Kontur");
  assert_eq!((figure, detail.key), (None, "overview.board.usageUnread"));
}

#[test]
fn board_has_no_notices_without_blocked_or_unreadable_tasks() {
  let (_fixture, tasks) = project();
  assert!(board::notices(&tasks, ", ").is_empty());
}
