//! The team scenarios: each is in the registry and leaves the marker the QA app reads its state from
//! (`examples/qa_app/team/state.rs`), every other scenario leaves none, `team-offer` registers a local project,
//! `team-outage-running`, `team-member-running` and `team-module-outage` have two tasks running on this computer,
//! `team-claims` a run of each task whose claim state rests on one, and `team-running` KON-12 interrupted at Implement.

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
    let scenario = scenarios::named(state.name());
    assert!(scenario.is_some(), "{state:?} is in the registry");
    // The seed only stores the fake Codex's program: any file stands in.
    let fake_codex = scenario
      .filter(|scenario| scenario.needs_fake_codex())
      .map(|_| std::path::PathBuf::from("Cargo.toml"));
    let seed = Seed::with(state.name(), fake_codex).expect("the scenario seeds");
    let read = State::beside(&seed.seeded.app_database).expect("the marker reads");
    assert_eq!(read, Some(state));
    let tasks = seed.db().tasks().expect("tasks").len();
    let expected = match state {
      State::Claims => 6,
      State::TeamRunning => 1,
      state if state.runs_here() => 2,
      _ => 0,
    };
    assert_eq!(tasks, expected, "{state:?}");
  }
}

#[test]
fn seed_team_claims_runs_each_task_its_claim_state_rests_on() {
  let seed = Seed::of("team-claims");
  let db = seed.db();
  for (key, expected) in [
    ("KON-12", TaskPhase::Running),
    ("KON-21", TaskPhase::Waiting),
    ("KON-11", TaskPhase::Cancelled),
    ("KON-24", TaskPhase::Interrupted),
  ] {
    assert_eq!(phase(&db, key), Some(expected), "{key}");
  }
}

#[test]
fn seed_team_running_states_run_kon_12_and_kon_21_here() {
  for name in ["team-outage-running", "team-member-running", "team-module-outage"] {
    let seed = Seed::of(name);
    let db = seed.db();
    for key in ["KON-12", "KON-21"] {
      assert_eq!(phase(&db, key), Some(TaskPhase::Running), "{name} {key}");
    }
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
