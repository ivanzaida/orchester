//! The team scenarios: each is in the registry and leaves the marker the QA app reads its state from
//! (`examples/qa_app/team/state.rs`), every other scenario leaves none, `team-offer` registers a local project, and
//! `team-outage-running` has two tasks running on this computer.

use orchester_core::{run::TaskPhase, store::TaskStore};
use orchester_store::AppDb;

use super::{
  Seed,
  kontur::scenarios::{self, team::state::State},
  phase,
};

#[test]
fn seed_team_scenarios_leave_the_marker_of_their_state() {
  for state in State::ALL {
    assert!(scenarios::named(state.name()).is_some(), "{state:?} is in the registry");
    let seed = Seed::of(state.name());
    let read = State::beside(&seed.seeded.app_database).expect("the marker reads");
    assert_eq!(read, Some(state));
    let tasks = seed.db().tasks().expect("tasks").len();
    let expected = if state == State::OutageRunning { 2 } else { 0 };
    assert_eq!(tasks, expected, "{state:?}");
  }
}

#[test]
fn seed_team_outage_running_runs_kon_12_and_kon_21_here() {
  let seed = Seed::of("team-outage-running");
  let db = seed.db();
  for key in ["KON-12", "KON-21"] {
    assert_eq!(phase(&db, key), Some(TaskPhase::Running), "{key}");
  }
}

#[test]
fn seed_team_no_marker_without_a_team_scenario() {
  let seed = Seed::of("empty");
  assert_eq!(State::beside(&seed.seeded.app_database).expect("nothing to read"), None);
}

#[test]
fn seed_team_offer_registers_ledger_as_a_local_project() {
  let seed = Seed::of("team-offer");
  let projects = AppDb::open(&seed.seeded.app_database)
    .expect("the application database")
    .every_project()
    .expect("the projects");
  let ledger = projects
    .iter()
    .find(|project| project.name.as_str() == "Ledger")
    .expect("Ledger is registered");
  assert!(ledger.repo_root.join(".git").is_dir(), "a repository of its own");
}
