//! The team scenarios: each is in the registry and leaves the marker the QA app reads its state from
//! (`examples/qa_app/team/state.rs`), every other scenario leaves none, `team-offer` registers a local project, and
//! `team-outage-running`, `team-member-running` and `team-module-outage` have two tasks running on this computer, in
//! `team-outage-running` with work reports waiting to be sent.

use orchester_core::{
  run::TaskPhase,
  store::{RunStore, TaskStore},
};
use orchester_store::{AppDb, WorkReportState};

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
    let expected = if state.runs_here() { 2 } else { 0 };
    assert_eq!(tasks, expected, "{state:?}");
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

/// `On this computer` (`XTBXg`): run 8 with 3 reports waiting, run 2 with 1.
#[test]
fn seed_team_outage_running_reports_wait_to_be_sent() {
  let seed = Seed::of("team-outage-running");
  let db = seed.db();
  let mut waiting: Vec<(u32, usize)> = db
    .team_runs()
    .expect("the team runs")
    .iter()
    .map(|team| {
      let run = db.run(team.run).expect("the run reads").expect("the run is recorded");
      let reports = db.work_reports(team.run).expect("the reports");
      let pending = reports
        .iter()
        .filter(|report| report.state == WorkReportState::Pending)
        .count();
      (run.number().get(), pending)
    })
    .collect();
  waiting.sort_unstable();
  assert_eq!(waiting, [(2, 1), (8, 3)]);
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
