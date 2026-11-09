//! The team scenarios: each is in the registry and leaves the marker the QA app reads its state from
//! (`examples/qa_app/team/state.rs`), every other scenario leaves none, `team-offer` registers a local project,
//! `team-outage-running`, `team-member-running` and `team-module-outage` have two tasks running on this computer (in
//! `team-outage-running` with work reports waiting to be sent), `team-claims` a run of each task whose claim state
//! rests on one, and `team-running` KON-12 interrupted at Implement. Those with runs here start on the seeded data as
//! the QA app starts them (`examples/qa_app/team`).

use std::path::PathBuf;

use orchester_app::{
  routes::{ProjectPage, Route, TaskView},
  test_support::{Data, Window, signed_in},
};
use orchester_core::{
  project::TaskKey,
  run::TaskPhase,
  store::{RunStore, TaskStore},
  task::identity::TaskOwner,
  time::Timestamp,
};
use orchester_store::{AppDb, TeamOperationState, WorkReportState};
use tokio::runtime::Runtime;

use super::{
  Seed, by_key,
  kontur::{
    self, SeedRequest,
    scenarios::{self, team::state::State},
  },
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
      State::TeamRunning | State::NotReadyAfterRun | State::TeamArchived => 1,
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
    ("KON-11", TaskPhase::Interrupted),
    ("KON-23", TaskPhase::Cancelled),
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
    let key = if state == State::NotReadyAfterRun {
      "KON-5"
    } else {
      "KON-12"
    };
    let owner = db
      .task_identity(by_key(&db)[key].id())
      .expect("the identity reads")
      .unwrap_or_else(|| panic!("{key} has an identity"))
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

/// `team-not-ready-ended`: KON-5's earlier run on this computer is run 8, cancelled.
#[test]
fn seed_team_not_ready_ended_cancels_kon_5s_run_8() {
  let seed = Seed::of("team-not-ready-ended");
  let db = seed.db();
  assert_eq!(phase(&db, "KON-5"), Some(TaskPhase::Cancelled));
  let runs = db.runs_of_task(by_key(&db)["KON-5"].id()).expect("the runs");
  let numbers: Vec<u32> = runs
    .iter()
    .map(|run| run.as_ref().expect("a readable run").number().get())
    .collect();
  assert_eq!(numbers, [8]);
}

/// 03 `hWXxr`: KON-11's run, interrupted and taken over by mara, stopped for good. Nothing continues or resumes it, in
/// the header or its *More actions*: an agent run on a task someone else holds would report nothing the server takes.
/// Cancel run stays, which stops an agent should one still work.
#[test]
fn seed_team_claims_a_fenced_interrupted_run_offers_no_continue() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let request = SeedRequest {
    data_dir: folder.path().to_path_buf(),
    repo: folder.path().join("repo-team-claims"),
    scenario: scenarios::named("team-claims").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = kontur::seed(&request).expect("the scenario seeds");
  let data = Data::at(folder, Runtime::new().expect("a runtime"));
  let launch = data.launch();
  let qa_state = qa_team::State::beside(&seeded.app_database)
    .expect("the marker reads")
    .expect("a team state");
  let services = launch.services().expect("the session started");
  let _server = qa_team::prepare(qa_state, services, data.runtime.handle()).expect("the state is set up");
  let mut window = Window::of(data, launch);
  let key: TaskKey = "KON-11".parse().expect("a task key");
  window.navigation().go(Route::Project(
    seeded.project,
    ProjectPage::task(key, TaskView::Summary, false),
  ));
  window.until("the run stopped", |window| {
    window
      .texts("task-status-label")
      .is_some_and(|texts| texts.concat() == "Taken over by mara")
  });
  let shows = |window: &mut Window, id: &str| window.tree.get_element_by_id(id).is_some();
  for id in ["task-continue-run", "task-resume-run", "task-team-continue"] {
    assert!(!shows(&mut window, id), "{id} in the header");
  }
  window.press("task-more");
  window.until("More actions", |window| shows(window, "task-cancel-run"));
  for id in ["task-continue-run", "task-resume-run"] {
    assert!(!shows(&mut window, id), "{id} in More actions");
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

/// A team resource is cached as a computer that syncs the team has it: at the server's version and with no draft of
/// its own. A seed that made it as a person's new resource left a draft based on nothing, which read as stale beside
/// v1 (ORC-848 D7).
#[test]
fn seed_team_hub_resources_are_the_servers_at_v1_with_no_draft() {
  use orchester_core::{
    resource::{AgentProfile, ResourceKind, VersionNumber},
    store::ResourceStore,
  };
  use orchester_workspace::agents::data::{ProjectAgents, VersionState};

  let seed = Seed::of("team-hub");
  let db = seed.db();
  let team: Vec<_> = db
    .resources(ResourceKind::AgentProfile)
    .expect("profiles")
    .into_iter()
    .filter(|resource| {
      matches!(
        db.resource_sharing(resource.id()).expect("sharing"),
        Some(orchester_store::ResourceSharing::Shared(_))
      )
    })
    .collect();
  assert!(!team.is_empty(), "the hub has team profiles");
  let listed = ProjectAgents::load_in(&seed.seeded.repo).expect("agents");
  for resource in team {
    let entry = listed.profile(resource.id()).expect("listed");
    assert_eq!(
      entry.state,
      VersionState::Published(VersionNumber::FIRST),
      "{} is the server's v1, not a draft",
      resource.name()
    );
    assert!(db.draft::<AgentProfile>(resource.id()).expect("read").is_none());
  }
}
