//! Agent CLI invocations and the transcript of a run.

mod support;

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, InvocationOutcome, OutputStream},
  run::TokenUsage,
  store::ActivityStore,
};
use rusqlite::Connection;

use self::support::{at, model, started_run};

#[test]
fn activity_invocations_are_numbered_per_execution_and_saved() {
  let (_repo, mut db, run, execution) = started_run();
  let mut first = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  assert_eq!(first.number, 1);
  first.session = Some("thread-1".into());
  first.usage = TokenUsage {
    input: 12_166,
    cached_input: 2_432,
    output: 5,
  };
  first.ended_at = Some(at(20));
  first.outcome = Some(InvocationOutcome::TurnFailed { message: "no".into() });
  db.save_invocation(&first).expect("save");
  let second = db
    .start_invocation(run.id(), execution, &model(), at(30))
    .expect("start");
  assert_eq!(second.number, 2);
  let stored = db.invocations(run.id()).expect("read");
  assert_eq!(stored, [first, second]);
  assert_eq!(stored[0].active_millis(at(99)), 10);
  assert_eq!(stored[1].active_millis(at(35)), 5);
}

#[test]
fn activity_transcript_round_trips_in_order_and_reads_after_a_sequence() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  let events = vec![
    (
      at(11),
      AgentEvent::SessionStarted {
        session: "thread-1".into(),
      },
    ),
    (at(12), AgentEvent::Message { text: "Working".into() }),
    (
      at(13),
      AgentEvent::CommandFinished {
        item: "item_1".into(),
        command: "git status".into(),
        output: "clean".into(),
        exit_code: Some(0),
        status: CommandStatus::Completed,
      },
    ),
  ];
  assert_eq!(db.append_activity(&invocation, &events[..2]).expect("append"), 2);
  assert_eq!(db.append_activity(&invocation, &events[2..]).expect("append"), 3);
  let raw = (
    at(14),
    AgentEvent::Raw {
      stream: OutputStream::Stderr,
      line: "warning".into(),
    },
  );
  assert_eq!(
    db.append_activity(&invocation, std::slice::from_ref(&raw))
      .expect("append"),
    4
  );
  let all = db.activity(run.id(), 0).expect("read");
  let read: Vec<_> = all.iter().map(|entry| (entry.at, entry.event.clone())).collect();
  let mut expected = events.clone();
  expected.push(raw);
  assert_eq!(read, expected);
  assert!(
    all
      .iter()
      .all(|entry| entry.invocation == invocation.id && entry.execution == execution)
  );
  let sequences: Vec<_> = db
    .activity(run.id(), 2)
    .expect("read")
    .iter()
    .map(|entry| entry.sequence)
    .collect();
  assert_eq!(sequences, [3, 4]);
}

#[test]
fn activity_transcript_is_append_only() {
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  db.append_activity(&invocation, &[(at(11), AgentEvent::TurnStarted)])
    .expect("append");
  let raw = Connection::open(repo.path().join(".orchester/orchester.db")).expect("open");
  assert!(raw.execute("UPDATE agent_events SET kind = 'x'", []).is_err());
  assert!(raw.execute("DELETE FROM agent_events", []).is_err());
}
