//! Auto mode in the project database: the project's setting for new runs and the reviewer's model, each
//! run's own choice taken from it when the run is created, and the agent's last messages the reviewer
//! reads. `cargo test -p orchester-store --test auto_mode auto_mode`.

mod support;

use std::time::Duration;

use orchester_core::{
  connection::stream::AgentEvent,
  id::IdempotencyKey,
  resource::ModelId,
  revision::Revision,
  run::{BranchName, NewRun, review::AutoModeSettings},
  store::{ActivityStore, RunStore},
};
use support::{at, budget, started_run};

#[test]
fn auto_mode_new_runs_take_the_projects_setting() {
  let (_repo, mut db, run, _) = started_run();
  assert_eq!(db.auto_mode_settings().expect("read"), AutoModeSettings::default());
  assert!(!AutoModeSettings::default().new_runs, "off until a person turns it on");
  assert!(!db.run_auto_mode(run.id()).expect("read"));

  let settings = AutoModeSettings {
    new_runs: true,
    model: ModelId::new("claude-sonnet-5-5").expect("model"),
  };
  db.save_auto_mode_settings(&settings).expect("save");
  assert_eq!(db.auto_mode_settings().expect("read"), settings);
  // A run created before keeps its own choice; a new one takes the setting.
  assert!(!db.run_auto_mode(run.id()).expect("read"));
  let next = NewRun {
    task: run.task(),
    snapshot: run.snapshot().clone(),
    branch: BranchName::new("orchester/orc-1-run-2").expect("branch"),
    base: Revision::new(&"b".repeat(40)).expect("revision"),
    base_branch: None,
    budget: budget(1_000_000),
    max_duration: Duration::from_secs(1_800),
  };
  let second = db
    .create_run(IdempotencyKey::generate(), next, at(20))
    .expect("second run");
  assert!(db.run_auto_mode(second.id()).expect("read"));
  db.set_run_auto_mode(second.id(), false).expect("turn off");
  assert!(!db.run_auto_mode(second.id()).expect("read"));
  db.set_run_auto_mode(run.id(), true).expect("turn on");
  assert!(db.run_auto_mode(run.id()).expect("read"));
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
