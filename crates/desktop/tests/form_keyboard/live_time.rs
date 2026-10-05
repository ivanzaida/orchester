//! Active agent time on the task page goes on while the agent works, headlessly with the real runner,
//! `orchester-fake-codex` and the harness's clock: the window's timers are run by hand at instants
//! the test picks, and the clock tells the time the test set. The value ticks with each second,
//! stops at the time limit, asks for no redraws while its window tab is offstage or once the run
//! paused, and then shows what the runner recorded.

use std::{
  cell::Cell,
  fs,
  time::{Duration, Instant},
};

use lurq::node::element::ElementRef;
use orchester_core::{run::active_time, store::ActivityStore, time::Timestamp};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{agent_step, create_and_run, has, init_repository, latest_run, programs, seed_steps},
};

/// The task page while Implement waits for the test's release, with Review after it.
fn working() -> Desktop {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      vec![
        agent_step(builder, "implement", "Implement", "Wait for release.", &[]),
        agent_step(builder, "review", "Review", "Append one line.", &["implement"]),
      ]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  desktop.wait_until("the agent at work", |desktop| {
    started_at(desktop).is_some() && has(desktop, "task-active-time")
  });
  desktop
}

/// When the run's first agent started, once it did.
fn started_at(desktop: &Desktop) -> Option<Timestamp> {
  let run = latest_run(desktop.repo())?;
  let db = ProjectDb::open(desktop.repo()).expect("open the project");
  let invocations = db.invocations(run.id()).expect("invocations");
  invocations.first().map(|invocation| invocation.started_at)
}

/// The run's recorded active agent time, as the page writes it.
fn recorded(desktop: &Desktop) -> String {
  let run = latest_run(desktop.repo()).expect("run");
  let db = ProjectDb::open(desktop.repo()).expect("open the project");
  let seconds = active_time(&db.invocations(run.id()).expect("invocations"), Timestamp::now()).as_secs();
  if seconds < 60 {
    format!("{seconds} s")
  } else {
    format!("{} min", seconds / 60)
  }
}

fn texts(element: ElementRef<'_>, found: &mut Vec<String>) {
  found.extend(element.text_content().map(str::to_owned));
  for child in element.children() {
    texts(child, found);
  }
}

/// The value of *Active agent time* in "This run".
fn shown(desktop: &mut Desktop) -> String {
  let mut found = Vec::new();
  texts(
    desktop
      .tree
      .get_element_by_id("task-active-time")
      .expect("the time is shown"),
    &mut found,
  );
  found.pop().expect("the row has its value")
}

/// Runs the window's timers as they would run a while later, then lays the window out.
fn tick(desktop: &mut Desktop, at: &Cell<Instant>) {
  at.set(at.get() + Duration::from_secs(2));
  desktop.tree.tick_timers_at(at.get());
  desktop.pass();
}

/// The clock reads `seconds` after the agent started.
fn after(desktop: &Desktop, started: Timestamp, seconds: i64) {
  desktop
    .clock
    .set(Timestamp::from_unix_millis(started.unix_millis() + seconds * 1000));
}

#[test]
fn live_time_ticks_while_the_agent_works_and_stops_at_the_limit_and_offstage() {
  let mut desktop = working();
  let started = started_at(&desktop).expect("the agent started");
  let at = Cell::new(Instant::now());
  after(&desktop, started, 5);
  desktop.wait_until("5 s of active agent time", |desktop| {
    tick(desktop, &at);
    shown(desktop) == "5 s"
  });
  for seconds in [6, 7] {
    desktop.clock.take_reads();
    after(&desktop, started, seconds);
    tick(&mut desktop, &at);
    assert_eq!(shown(&mut desktop), format!("{seconds} s"));
    assert!(desktop.clock.take_reads() > 0, "the tick asked for a redraw");
  }
  // The working step's row counts on with it.
  assert!(shows(&mut desktop, "task-progress-steps", "· 7 s"));

  // In a tab that is offstage nothing ticks; back on stage it goes on.
  desktop.press("window-new-tab");
  desktop.wait_for("all-projects");
  desktop.clock.take_reads();
  for _ in 0..3 {
    tick(&mut desktop, &at);
  }
  assert_eq!(desktop.clock.take_reads(), 0, "no redraws while the tab is offstage");
  desktop.press("window-tab-1");
  desktop.wait_for("task-active-time");
  after(&desktop, started, 8);
  tick(&mut desktop, &at);
  assert_eq!(shown(&mut desktop), "8 s");

  // Two hours on the clock read as the run's 30 minutes: the runner stops its agents there.
  after(&desktop, started, 2 * 60 * 60);
  tick(&mut desktop, &at);
  assert_eq!(shown(&mut desktop), "30 min");
  fs::write(desktop.repo().join(".orchester").join("fake-release"), "go").expect("release");
}

#[test]
fn live_time_stops_when_the_run_pauses_and_shows_what_was_recorded() {
  let mut desktop = working();
  let started = started_at(&desktop).expect("the agent started");
  let at = Cell::new(Instant::now());
  after(&desktop, started, 40);
  desktop.wait_until("40 s of active agent time", |desktop| {
    tick(desktop, &at);
    shown(desktop) == "40 s"
  });
  desktop.click("task-pause-run");
  fs::write(desktop.repo().join(".orchester").join("fake-release"), "go").expect("release");
  desktop.wait_until("Resume run", |desktop| has(desktop, "task-resume-run"));
  // The run read again replaces what the page counted with what the runner recorded.
  let recorded = recorded(&desktop);
  assert_eq!(shown(&mut desktop), recorded);
  desktop.clock.take_reads();
  for seconds in [41, 42, 43] {
    after(&desktop, started, seconds);
    tick(&mut desktop, &at);
  }
  assert_eq!(shown(&mut desktop), recorded, "a paused run's time stands");
  assert_eq!(desktop.clock.take_reads(), 0, "no redraws while nothing counts");
}
