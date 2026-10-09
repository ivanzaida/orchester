//! A long transcript: KON-12 of the `conversation` scenario with 2,000 more entries, half messages and half
//! commands, opens at its end, following it, within a time a person waits for; the list builds the rows in
//! and near its view, not every row.

use std::time::{Duration, Instant};

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus},
  store::{ActivityStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{ids, key_of, open, seeded};

/// Messages and commands, one each per turn.
const TURNS: usize = 1_000;
/// How long opening the chat may take in a debug build.
const OPENS_WITHIN: Duration = Duration::from_secs(30);

#[test]
fn timeline_a_long_transcript_opens_at_its_end_without_building_every_row() {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, "Add retry to webhook sender");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project database");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| *task.key() == key)
    .expect("KON-12");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .filter_map(Result::ok)
    .next_back()
    .expect("its latest run");
  let invocation = db
    .invocations(run.id())
    .expect("invocations")
    .into_iter()
    .rev()
    .find(|invocation| invocation.ended_at.is_none())
    .expect("its agent at work");
  let start = Timestamp::now().unix_millis();
  let events: Vec<(Timestamp, AgentEvent)> = (0..TURNS)
    .flat_map(|turn| {
      let at = |offset: i64| Timestamp::from_unix_millis(start + i64::try_from(turn).expect("few turns") * 3 + offset);
      let item = format!("long-{turn}");
      let command = format!("cargo test case_{turn}");
      [
        (
          at(0),
          AgentEvent::Message {
            text: format!("Turn {turn} done."),
          },
        ),
        (
          at(1),
          AgentEvent::CommandStarted {
            item: item.clone(),
            command: command.clone(),
          },
        ),
        (
          at(2),
          AgentEvent::CommandFinished {
            item,
            command,
            output: "ok".to_owned(),
            exit_code: Some(0),
            status: CommandStatus::Completed,
          },
        ),
      ]
    })
    .collect();
  db.append_activity(&invocation, &events).expect("the turns are added");
  let opened = Instant::now();
  open(&mut window, &seeded, &key, "conversation-chat");
  let last = format!("Turn {} done.", TURNS - 1);
  window.until("the last turn in view", |window| {
    window
      .texts("conversation")
      .is_some_and(|texts| texts.iter().any(|text| text.contains(&last)))
  });
  let took = opened.elapsed();
  assert!(took < OPENS_WITHIN, "opening 2,000 entries took {took:?}");
  let built = ids(&mut window, "conversation-row:").len();
  assert!(
    built < 200,
    "the list builds the rows near its view, not all of them: {built}"
  );
}
