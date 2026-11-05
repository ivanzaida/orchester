//! The team scenarios: each is in the registry and leaves the marker the QA app reads its state from
//! (`examples/qa_app/team/state.rs`), every other scenario leaves none, `team-offer` registers a local project,
//! `team-outage-running`, `team-member-running` and `team-module-outage` have two tasks running on this computer (in
//! `team-outage-running` with work reports waiting to be sent), `team-claims` a run of each task whose claim state
//! rests on one, and `team-running` KON-12 interrupted at Implement. Those with runs here start on the seeded data as
//! the QA app starts them (`examples/qa_app/team`).

use std::path::PathBuf;

use orchester_app::test_support::signed_in;
use orchester_core::{
  run::TaskPhase,
  store::{RunStore, TaskStore},
  task::identity::TaskOwner,
};
use orchester_store::{AppDb, TeamOperationState, WorkReportState};
use tokio::runtime::Runtime;

use super::{
  Seed, by_key,
  kontur::scenarios::{self, team::state::State},
  phase, qa_team,
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

/// What the QA app does with the seeded data of each scenario with runs here before its window opens (ORC-732 Verify
/// found three of them refused at startup): it joins the stand-in team and claims the seeded tasks through the
/// dispatcher, which takes them as the team's, never as this computer's own. In `team-running` KON-12's work starts
/// through the stand-in's `work.start`, so its team run reports under the operation and work the stand-in issued.
#[test]
fn seed_team_scenarios_with_runs_here_start_as_the_qa_app_starts_them() {
  for state in State::ALL.into_iter().filter(|state| state.runs_here()) {
    let scenario = scenarios::named(state.name()).expect("a known scenario");
    // The seed only stores the fake Codex's program: any file stands in.
    let fake_codex = scenario.needs_fake_codex().then(|| PathBuf::from("Cargo.toml"));
    let seed = Seed::with(state.name(), fake_codex).expect("the scenario seeds");
    let database = &seed.seeded.app_database;
    let runtime = Runtime::new().expect("a runtime");
    let launch = signed_in::launch(database, database.with_file_name("no-runner"), runtime.handle());
    let services = launch.services().expect("the session started");
    let qa_state = qa_team::State::beside(database)
      .expect("the marker reads")
      .expect("a team state");
    let server = match qa_team::prepare(qa_state, services, runtime.handle()) {
      Ok(server) => server,
      Err(error) => panic!("{state:?} starts: {error}"),
    };
    let db = seed.db();
    let owner = db
      .task_identity(by_key(&db)["KON-12"].id())
      .expect("the identity reads")
      .expect("KON-12 has an identity")
      .owner;
    let claimed = state != State::OutageRunning;
    assert_eq!(matches!(owner, TaskOwner::Team { .. }), claimed, "{state:?}");
    if state == State::TeamRunning {
      assert!(
        server.commands().iter().any(|kind| kind == "work.start"),
        "the stand-in started the work"
      );
      let runs = db.team_runs().expect("the team runs");
      let [run] = runs.as_slice() else {
        panic!("one team run: {runs:?}");
      };
      let start = db
        .team_operation(run.operation)
        .expect("the operation reads")
        .expect("the start is the dispatcher's operation");
      assert_eq!(
        start.state,
        TeamOperationState::Completed,
        "its run carries its effect out"
      );
      assert_eq!(run.fence, None, "it reports");
    }
    launch.end();
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
