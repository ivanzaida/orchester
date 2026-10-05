//! Active agent time as a window shows it live between two reads of a run: it counts on only while
//! an invocation runs without waiting for a person and the run runs, never past the time limit.

use std::time::Duration;

use orchester_core::{
  connection::stream::InvocationOutcome,
  id::{InvocationId, StepExecutionId},
  run::{ActiveClock, Invocation, InvocationKind, InvocationTiming, RunLimit, TokenUsage},
  tokens::TokensUsed,
};

use crate::fixture::{Logged, at, model, revision};

const MINUTE: Duration = Duration::from_secs(60);
const MINUTE_MILLIS: i64 = 60_000;

/// A run whose `research` step is running, with its time limit of 30 minutes.
fn working() -> Logged {
  let mut logged = Logged::started();
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged
}

/// An invocation of `execution` from minute `started` to minute `ended`, or still running.
fn invocation(logged: &Logged, execution: StepExecutionId, started: i64, ended: Option<i64>) -> Invocation {
  Invocation {
    id: InvocationId::generate(),
    run: logged.run.id(),
    execution,
    number: 1,
    kind: InvocationKind::Agent {
      model: model("gpt-5.5"),
    },
    permissions: None,
    session: None,
    started_at: at(started * MINUTE_MILLIS),
    ended_at: ended.map(|ended| at(ended * MINUTE_MILLIS)),
    usage: TokenUsage::default(),
    outcome: ended.map(|_| InvocationOutcome::Completed),
    timing: InvocationTiming::default(),
  }
}

#[test]
fn active_clock_counts_on_from_what_was_read_while_an_agent_works() {
  let logged = working();
  let research = logged.exec("research");
  let invocations = [
    invocation(&logged, research, 0, Some(10)),
    invocation(&logged, research, 20, None),
  ];
  let clock = ActiveClock::of(&invocations, at(22 * MINUTE_MILLIS)).within(&logged.run);
  assert!(clock.counting());
  assert_eq!(clock.at(at(22 * MINUTE_MILLIS)), MINUTE * 12);
  assert_eq!(clock.at(at(25 * MINUTE_MILLIS)), MINUTE * 15);
  // A clock set back reads what was read, never less.
  assert_eq!(clock.at(at(21 * MINUTE_MILLIS)), MINUTE * 12);
  // Never past the time limit, at which the runner stops the agent.
  assert_eq!(clock.at(at(90 * MINUTE_MILLIS)), MINUTE * 30);
  let read_late = ActiveClock::of(&invocations, at(90 * MINUTE_MILLIS)).within(&logged.run);
  assert_eq!(read_late.at(at(90 * MINUTE_MILLIS)), MINUTE * 30);
  // Without a runner to look after the agent, what was read stays.
  assert!(!clock.stopped().counting());
  assert_eq!(clock.stopped().at(at(25 * MINUTE_MILLIS)), MINUTE * 12);
}

#[test]
fn active_clock_counts_each_agent_at_work_as_the_time_limit_does() {
  let logged = working();
  let research = logged.exec("research");
  let invocations = [
    invocation(&logged, research, 0, None),
    invocation(&logged, StepExecutionId::generate(), 0, None),
  ];
  let clock = ActiveClock::of(&invocations, at(MINUTE_MILLIS)).within(&logged.run);
  assert_eq!(clock.at(at(MINUTE_MILLIS)), MINUTE * 2);
  assert_eq!(clock.at(at(3 * MINUTE_MILLIS)), MINUTE * 6);
}

#[test]
fn active_clock_stops_while_the_agent_waits_for_a_person() {
  let logged = working();
  let mut waiting = invocation(&logged, logged.exec("research"), 0, None);
  waiting.timing.wait_for_person(at(4 * MINUTE_MILLIS));
  let clock = ActiveClock::of([&waiting], at(6 * MINUTE_MILLIS)).within(&logged.run);
  assert!(!clock.counting());
  assert_eq!(clock.at(at(20 * MINUTE_MILLIS)), MINUTE * 4);
}

#[test]
fn active_clock_stops_once_the_run_stops_running() {
  let mut logged = working();
  let research = logged.exec("research");
  logged.apply(|run| run.record_usage(TokensUsed::new(1_400_000), at(13)));
  logged.apply(|run| run.record_checkpoint(research, revision('d'), at(14)));
  logged.apply(|run| run.stop_at_limit(RunLimit::Tokens, at(15)));
  // The runner had not yet recorded the agent's end when the run was read.
  let open = invocation(&logged, research, 0, None);
  let clock = ActiveClock::of([&open], at(5 * MINUTE_MILLIS)).within(&logged.run);
  assert!(!clock.counting());
  assert_eq!(clock.at(at(9 * MINUTE_MILLIS)), MINUTE * 5);
}
