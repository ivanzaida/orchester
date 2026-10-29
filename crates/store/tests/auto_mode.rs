//! Manual's reviewer in the project database: the reviewer a person chose, where origin pushed as a run started, and
//! the agent's last messages the reviewer reads. `cargo test -p orchester-store --test auto_mode auto_mode`.

mod support;

use orchester_core::{
  connection::stream::AgentEvent,
  id::ConnectionId,
  resource::ModelId,
  run::review::{AutoModeSettings, ReviewerChoice},
  store::ActivityStore,
};
use orchester_store::AutoModeSave;
use support::{at, started_run};

#[test]
fn auto_mode_keeps_the_reviewer_a_person_chose_and_the_runs_origin() {
  let (_repo, mut db, run, _) = started_run();
  assert_eq!(db.auto_mode_settings().expect("read"), AutoModeSettings::default());

  let settings = AutoModeSettings {
    reviewer: Some(ReviewerChoice {
      connection: ConnectionId::generate(),
      model: ModelId::new("claude-sonnet-5-5").expect("model"),
      effort: Some("high".to_owned()),
    }),
  };
  db.save_auto_mode_settings(&settings).expect("save");
  assert_eq!(db.auto_mode_settings().expect("read"), settings);
  // Back to the default reviewer: no connection, model or effort is read.
  let default = AutoModeSettings::default();
  db.save_auto_mode_settings(&default).expect("save");
  assert_eq!(db.auto_mode_settings().expect("read"), default);
  // Where origin pushed as the run started.
  assert_eq!(db.run_origin(run.id()).expect("read"), None);
  db.record_run_origin(run.id(), Some("https://example.invalid/kontur.git"))
    .expect("record");
  assert_eq!(
    db.run_origin(run.id()).expect("read").as_deref(),
    Some("https://example.invalid/kontur.git")
  );
}

#[test]
fn auto_mode_reads_the_agents_last_messages_oldest_first() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &support::model(), at(10))
    .expect("invocation");
  let events: Vec<_> = (1..=4)
    .map(|number| {
      (
        at(10 + number),
        AgentEvent::Message {
          text: format!("message {number}"),
        },
      )
    })
    .chain([(at(20), AgentEvent::TurnStarted)])
    .collect();
  db.append_activity(&invocation, &events).expect("activity");
  assert_eq!(
    db.recent_agent_messages(run.id(), execution, 3).expect("messages"),
    ["message 2", "message 3", "message 4"]
  );
}

#[test]
fn auto_mode_update_and_compare_and_save_change_what_is_stored() {
  let (_repo, mut db, ..) = started_run();
  let reviewer = ReviewerChoice {
    connection: ConnectionId::generate(),
    model: ModelId::new("claude-sonnet-5-5").expect("model"),
    effort: Some("high".to_owned()),
  };
  let chosen = AutoModeSettings {
    reviewer: Some(reviewer),
  };
  let saved = db.update_auto_mode_settings(|_| chosen.clone()).expect("update");
  assert_eq!(saved, chosen);
  assert_eq!(db.auto_mode_settings().expect("read"), saved);
  // A save from a stale base is refused and changes nothing; one from the stored base is taken.
  let default = AutoModeSettings::default();
  assert_eq!(
    db.save_auto_mode_if_current(&default, &default).expect("save"),
    AutoModeSave::ChangedOutside
  );
  assert_eq!(db.auto_mode_settings().expect("read"), chosen);
  assert_eq!(
    db.save_auto_mode_if_current(&chosen, &default).expect("save"),
    AutoModeSave::Saved
  );
  assert_eq!(db.auto_mode_settings().expect("read"), default);
}
