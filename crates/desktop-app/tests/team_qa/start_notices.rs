//! What a team task's page tells of its start apart from a press of Run (ORC-885c): a start that waits for the team
//! server or the runner is the information notice `xnJyn` with nothing to press; a runner's refusal that recovery found
//! is told as a Run that met it would; a failure that was settled in an earlier window is told once, with *Dismiss*,
//! and not again once dismissed, nor after a new window opens. On the QA states `team-start-waiting`,
//! `team-start-runner-refused` and `team-start-past-failure`, where KON-5's start is recorded when the project opens.
//!
//! `cargo test -p orchester-app --features test-support --test team_qa team_qa_start_notices`

use std::{
  path::Path,
  time::{Duration, Instant},
};

use orchester_app::test_support::Window;
use orchester_core::id::OperationId;

use super::{
  not_ready::on_task,
  start_failed::{WITHIN, notice, notice_says, open_starts, reopened, starting, waiting_run},
  team::State,
};

fn pressable(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

#[test]
fn team_qa_start_notices_a_start_that_waits_is_an_information_notice_with_nothing_to_press() {
  let (mut window, _server, _project) = waiting_run();

  assert!(starting(&mut window));
  assert!(window.text("task-team-starting").starts_with("Starting KON-5's run…"));
  assert!(window.texts("task-team-refused").is_none(), "it is not a failure");
  assert!(
    !pressable(&mut window, "task-team-refused-again"),
    "it retries on its own"
  );
  assert!(!pressable(&mut window, "task-team-refused-dismiss"));
}

#[test]
fn team_qa_start_notices_a_recorded_start_that_waits_says_so_without_a_press_of_run() {
  let (mut window, _server, project) = on_task(("team-start-waiting", State::StartWaiting), "KON-5");
  window.until_within(WITHIN, "the notice", starting);
  assert!(window.text("task-team-starting").starts_with("Starting KON-5's run…"));
  assert!(window.texts("task-team-refused").is_none());

  // A window opened later says it too: a start that waits stands for as long as it waits.
  let mut window = reopened(window, project);
  window.until_within(WITHIN, "the notice again", starting);
}

#[test]
fn team_qa_start_notices_a_runner_refusal_recovery_found_is_told_as_a_run_that_met_it_would() {
  let (mut window, _server, project) = on_task(("team-start-runner-refused", State::StartRunnerRefused), "KON-5");
  let root = window.data.repo("repo-kontur");

  window.until_within(WITHIN, "the runner's refusal", |window| {
    notice_says(window, "The runner reported a problem while starting the run")
  });
  assert!(
    window.texts("task-team-starting").is_none(),
    "it no longer says it is starting"
  );
  assert!(pressable(&mut window, "task-team-refused-again"));
  assert!(
    !pressable(&mut window, "task-team-refused-dismiss"),
    "the start is not settled"
  );
  assert_eq!(open_starts(&root), 1, "the runner is asked again");

  // It stays for a window opened later: the refusal holds for as long as the runner says it.
  let mut window = reopened(window, project);
  window.until_within(WITHIN, "the refusal again", |window| {
    notice_says(window, "The runner reported a problem while starting the run")
  });
}

#[test]
fn team_qa_start_notices_a_failure_settled_earlier_is_told_once_and_dismissed() {
  let (mut window, _server, project) = on_task(("team-start-past-failure", State::StartPastFailure), "KON-5");

  window.until_within(WITHIN, "the failure", |window| notice_says(window, "Update Orchester"));
  assert!(
    notice_says(&mut window, "Your claim on the task was released."),
    "{}",
    notice(&mut window)
  );
  assert!(pressable(&mut window, "task-team-refused-again"));
  assert!(pressable(&mut window, "task-team-refused-dismiss"));

  window.press("task-team-refused-dismiss");
  window.until_within(WITHIN, "the notice goes", |window| {
    window.texts("task-team-refused").is_none()
  });
  let root = window.data.repo("repo-kontur");
  window.until_within(WITHIN, "the start is marked seen", |_| {
    rusqlite::Connection::open(root.join(".orchester/orchester.db"))
      .expect("open")
      .query_row("SELECT COUNT(*) FROM team_start_seen", [], |row| row.get::<_, i64>(0))
      .expect("count")
      == 1
  });

  // A window opened after it does not tell it again.
  let mut window = reopened(window, project);
  window.until_within(WITHIN, "the task", |window| window.texts("task-run").is_some());
  let calm = std::time::Instant::now() + std::time::Duration::from_secs(3);
  window.until_within(WITHIN, "the page settles", |_| std::time::Instant::now() > calm);
  assert!(
    window.texts("task-team-refused").is_none(),
    "{:?}",
    window.texts("task-team-refused")
  );
  assert!(window.texts("task-team-starting").is_none());
}

/// Records a start of KON-5 newer than the one the project holds, as `stage` and `failure` say, copying its request.
fn record_newer_start(root: &Path, (stage, failure): (&str, Option<&str>)) {
  let created = orchester_core::time::Timestamp::now().unix_millis();
  rusqlite::Connection::open(root.join(".orchester/orchester.db"))
    .expect("open")
    .execute(
      "INSERT INTO team_starts (operation_id, task_id, request, stage, work_id, bundle_sha256, failure, created_at, \
       updated_at) SELECT ?1, task_id, request, ?2, work_id, bundle_sha256, ?3, ?4, ?4 FROM team_starts \
       ORDER BY created_at DESC LIMIT 1",
      rusqlite::params![OperationId::generate().to_string(), stage, failure, created],
    )
    .expect("a newer start");
}

fn seen_marks(root: &Path) -> i64 {
  rusqlite::Connection::open(root.join(".orchester/orchester.db"))
    .expect("open")
    .query_row("SELECT COUNT(*) FROM team_start_seen", [], |row| row.get(0))
    .expect("count")
}

fn failure_of(root: &Path) -> String {
  rusqlite::Connection::open(root.join(".orchester/orchester.db"))
    .expect("open")
    .query_row("SELECT failure FROM team_starts", [], |row| row.get(0))
    .expect("the failure")
}

#[test]
fn team_qa_start_notices_dismissing_marks_the_start_on_screen_and_not_a_newer_one() {
  let (mut window, _server, _project) = on_task(("team-start-past-failure", State::StartPastFailure), "KON-5");
  let root = window.data.repo("repo-kontur");
  window.until_within(WITHIN, "the failure", |window| notice_says(window, "Update Orchester"));

  // Another start of the task failed meanwhile, which the page does not show yet.
  record_newer_start(&root, ("failed", Some(&failure_of(&root))));
  window.press("task-team-refused-dismiss");
  let calm = Instant::now() + Duration::from_secs(3);
  window.until_within(WITHIN, "the page settles", |_| Instant::now() > calm);

  assert_eq!(seen_marks(&root), 0, "the newer start was not marked seen");
  assert!(
    notice_says(&mut window, "Update Orchester"),
    "the newer start tells its own failure, which nobody has seen"
  );
}

#[test]
fn team_qa_start_notices_a_notice_after_a_dismissal_shows_in_the_same_window() {
  let (mut window, _server, _project) = on_task(("team-start-past-failure", State::StartPastFailure), "KON-5");
  let root = window.data.repo("repo-kontur");
  window.until_within(WITHIN, "the failure", |window| notice_says(window, "Update Orchester"));
  window.press("task-team-refused-dismiss");
  window.until_within(WITHIN, "the notice goes", |window| {
    window.texts("task-team-refused").is_none()
  });
  window.until_within(WITHIN, "the start is marked seen", |_| seen_marks(&root) == 1);

  // Recovery finds a new start the runner refuses: the same window tells it, as it would without the dismissal.
  record_newer_start(&root, ("inputs_stored", None));
  window.until_within(WITHIN, "the runner's refusal", |window| {
    notice_says(window, "The runner reported a problem while starting the run")
  });
}
