//! The Overview's sections as data (`orchester_workspace::overview`) on readings and states made here: plan limits
//! shown only near or at a limit, a window that reset since its reading kept greyed; the getting-started steps marked
//! done from what the project has; a queued task's place in line.

use orchester_core::{
  connection::{
    AgentProvider,
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
  },
  id::AgentAccountId,
  time::Timestamp,
};
use orchester_workspace::{
  overview::{
    limits::{LimitLook, near_limits},
    queue::place,
    start::{StartStep, done, steps},
  },
  tasks::ready_to_run::ProjectReadiness,
};

const HOUR: i64 = 3_600_000;

/// A reading of `account` at `at`, its five-hour window `used` percent full and resetting at `resets`.
fn reading(account: AgentAccountId, at: i64, used: f64, resets: i64) -> LimitReading {
  let window = LimitWindow::new(
    300,
    Some(used),
    Some(Timestamp::from_unix_millis(resets)),
    LimitState::Normal,
    false,
  )
  .expect("a valid window");
  LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    account: Some(account),
    at: Timestamp::from_unix_millis(at),
    limits: PlanLimits {
      windows: vec![window],
      plan: None,
      credits: None,
      blocked: None,
    },
  }
}

#[test]
fn overview_plan_limits_show_only_accounts_near_or_at_a_limit_and_keep_a_reset_one_greyed() {
  let (quiet, near, reached, reset) = (
    AgentAccountId::generate(),
    AgentAccountId::generate(),
    AgentAccountId::generate(),
    AgentAccountId::generate(),
  );
  let now = 10 * HOUR;
  let mut readings = PlanReadings::default();
  for reading in [
    reading(quiet, now - HOUR, 40.0, now + HOUR),
    reading(near, now - HOUR, 86.0, now + HOUR),
    reading(reached, now - HOUR, 100.0, now + HOUR),
    // Read at 91 %, and its window reset before now: its use now is unknown.
    reading(reset, now - 3 * HOUR, 91.0, now - HOUR),
  ] {
    assert!(readings.observe(reading), "a reading of its own account");
  }
  let shown = near_limits(
    &[quiet, near, reached, reset],
    &readings,
    Timestamp::from_unix_millis(now),
  );
  let looks: Vec<(AgentAccountId, LimitLook)> = shown.iter().map(|limit| (limit.account, limit.look)).collect();
  assert_eq!(
    looks,
    [
      (near, LimitLook::Near),
      (reached, LimitLook::Reached),
      (reset, LimitLook::ResetSince),
    ],
    "the quiet account stays off; the reset one is never hidden"
  );
  let greyed = &shown[2];
  assert_eq!(
    greyed.window.as_ref().and_then(LimitWindow::percent),
    Some(91.0),
    "what it last read"
  );
}

#[test]
fn overview_getting_started_marks_steps_done_from_what_the_project_has() {
  let nothing = steps(&ProjectReadiness::default(), 0, ", ");
  assert_eq!(done(&nothing), 0);
  assert_eq!(nothing.map(|line| line.step), StartStep::ALL);
  let set_up = ProjectReadiness {
    agents: vec!["Builder".into()],
    pipelines: vec![(
      "Feature delivery".into(),
      orchester_core::resource::VersionNumber::FIRST,
    )],
    connections: vec![(
      "Claude Code".into(),
      orchester_core::connection::ConnectionStatus::Connected,
    )],
  };
  let lines = steps(&set_up, 0, ", ");
  assert_eq!(lines.map(|line| line.done), [true, true, true, false], "no task yet");
  assert_eq!(done(&steps(&set_up, 3, ", ")), 4);
}

#[test]
fn overview_queue_places_read_as_people_say_them() {
  let words: Vec<(&str, Option<String>)> = [1, 2, 3, 4, 11, 21, 22, 113]
    .into_iter()
    .map(|number| {
      let message = place(number);
      (message.key, message.args.first().map(|(_, value)| value.clone()))
    })
    .collect();
  assert_eq!(
    words,
    [
      ("overview.queue.place.next", None),
      ("overview.queue.place.nd", Some("2".to_owned())),
      ("overview.queue.place.rd", Some("3".to_owned())),
      ("overview.queue.place.th", Some("4".to_owned())),
      ("overview.queue.place.th", Some("11".to_owned())),
      ("overview.queue.place.st", Some("21".to_owned())),
      ("overview.queue.place.nd", Some("22".to_owned())),
      ("overview.queue.place.th", Some("113".to_owned())),
    ]
  );
}
