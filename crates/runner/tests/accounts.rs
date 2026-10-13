//! Agents run on their connection's account: the CLI starts with `CLAUDE_CONFIG_DIR` / `CODEX_HOME` set to the
//! account's folder, Default leaves the environment's, and no run starts without an account or its folder. End to
//! end against the fake CLIs. Run with `cargo test -p orchester-runner --features test-agent --test accounts`.

mod support;

use std::{num::NonZeroU32, path::Path};

use orchester_core::{
  account::AccountCli,
  connection::{AccountStartProblem, AgentConnection, AgentProvider, ConnectionAccount, ConnectionCheck, LastCheck},
  id::{AgentAccountId, IdempotencyKey},
  resource::StepKey,
  run::{PauseReason, PreflightProblem, RunLimit, RunStatus},
  store::{ActivityStore, ConnectionStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::{AttachSession, Event, Rejection};
use orchester_store::ProjectDb;
use tempfile::TempDir;

use self::support::{Setup, TestProject, project, reported, scheduler, wait_for};

fn on(provider: AgentProvider) -> TestProject {
  project(&Setup {
    provider,
    ..Setup::new("Append one line.")
  })
}

/// Puts the project's connection on `account`, keeping its check as made on that account.
fn choose(root: &Path, account: ConnectionAccount) {
  let mut db = ProjectDb::open(root).expect("open");
  let stored = db.connections().expect("connections").remove(0);
  let check: Option<ConnectionCheck> = match stored.last_check() {
    Some(LastCheck::Agent(check)) => Some(check.clone()),
    _ => None,
  };
  let mut connection = AgentConnection::try_from(stored).expect("an agent connection");
  connection.set_account(account).expect("an agent CLI");
  if let Some(check) = check {
    connection.record_check(check).expect("its kind");
  }
  db.save_connection(&connection.into_connection()).expect("saved");
}

async fn folder_the_agent_saw(provider: AgentProvider, account: Option<&TempDir>) -> Vec<String> {
  let project = on(provider);
  let root = project.repo.path();
  if let Some(folder) = account {
    choose(
      root,
      ConnectionAccount::Account {
        id: AgentAccountId::generate(),
        folder: folder.path().to_path_buf(),
      },
    );
  }
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  reported(&ProjectDb::open(root).expect("open"), id, "account folder: ")
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_claude_code_runs_with_its_accounts_config_folder() {
  let folder = tempfile::tempdir().expect("an account folder");
  let seen = folder_the_agent_saw(AgentProvider::ClaudeCodeCli, Some(&folder)).await;
  assert_eq!(seen, [folder.path().display().to_string()]);
  assert_eq!(AccountCli::ClaudeCode.folder_variable(), "CLAUDE_CONFIG_DIR");
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_codex_runs_with_its_accounts_home() {
  let folder = tempfile::tempdir().expect("an account folder");
  let seen = folder_the_agent_saw(AgentProvider::CodexCli, Some(&folder)).await;
  assert_eq!(seen, [folder.path().display().to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_default_leaves_the_folder_as_the_environment_has_it() {
  let seen = folder_the_agent_saw(AgentProvider::ClaudeCodeCli, None).await;
  let inherited: Vec<String> = std::env::var_os("CLAUDE_CONFIG_DIR")
    .map(|folder| folder.to_string_lossy().into_owned())
    .into_iter()
    .collect();
  assert_eq!(seen, inherited);
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_no_run_starts_without_an_account_or_its_folder() {
  let project = on(AgentProvider::ClaudeCodeCli);
  let root = project.repo.path();
  let step = StepKey::new("implement").expect("key");
  choose(root, ConnectionAccount::Unassigned);
  let (scheduler, _events) = scheduler(root);
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), project.task).await,
    Err(Rejection::Preflight {
      problems: vec![PreflightProblem::AccountUnavailable {
        step: step.clone(),
        reason: AccountStartProblem::NoAccount,
      }]
    })
  );
  let gone = root.join("no-such-account-folder");
  choose(
    root,
    ConnectionAccount::Account {
      id: AgentAccountId::generate(),
      folder: gone.clone(),
    },
  );
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), project.task).await,
    Err(Rejection::Preflight {
      problems: vec![PreflightProblem::AccountUnavailable {
        step,
        reason: AccountStartProblem::FolderMissing { folder: gone },
      }]
    })
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_plan_limits_and_invocations_name_the_account() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Report plan limits")
  });
  let root = project.repo.path();
  let folder = tempfile::tempdir().expect("an account folder");
  let account = AgentAccountId::generate();
  choose(
    root,
    ConnectionAccount::Account {
      id: account,
      folder: folder.path().to_path_buf(),
    },
  );
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let reading = tokio::time::timeout(support::PATIENCE, async {
    loop {
      if let Some(Event::PlanLimits(reading)) = events.recv().await {
        break reading;
      }
    }
  })
  .await
  .expect("a reading");
  assert_eq!(reading.account, Some(account));
  wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  let db = ProjectDb::open(root).expect("open");
  let history = db.limit_snapshots(Timestamp::from_unix_millis(0)).expect("history");
  assert_eq!(history[0].account, Some(account));
  let invocations = db.invocation_accounts(id).expect("accounts");
  assert_eq!(invocations.len(), 1);
  assert_eq!(invocations[0].1, account);
}

/// A Claude Code project on account `Work` whose folder has no room left, and a second connection of the CLI on
/// `Personal`, whose folder has.
fn at_the_limit() -> (TestProject, TempDir, TempDir, orchester_core::id::ConnectionId) {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Hit the plan limit")
  });
  let root = project.repo.path();
  let work = tempfile::tempdir().expect("Work's folder");
  let personal = tempfile::tempdir().expect("Personal's folder");
  std::fs::write(personal.path().join("plan-room"), "").expect("Personal has room");
  choose(
    root,
    ConnectionAccount::Account {
      id: AgentAccountId::generate(),
      folder: work.path().to_path_buf(),
    },
  );
  let mut other =
    AgentConnection::try_from(support::agent_connection_of(AgentProvider::ClaudeCodeCli)).expect("an agent connection");
  other.rename(orchester_core::name::Name::new("Claude · Personal").expect("a name"));
  other
    .set_account(ConnectionAccount::Account {
      id: AgentAccountId::generate(),
      folder: personal.path().to_path_buf(),
    })
    .expect("an agent CLI");
  other
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: orchester_core::connection::CheckResult::Available {
        version: "2.1.89".into(),
        sign_in: orchester_core::connection::SignIn::SignedIn,
        catalog: orchester_core::connection::ModelCatalog::new(
          orchester_core::connection::CatalogSource::Provider,
          Vec::new(),
        ),
      },
    })
    .expect("its kind");
  let id = other.id();
  ProjectDb::open(root)
    .expect("open")
    .create_connection(&other.into_connection())
    .expect("created");
  (project, work, personal, id)
}

/// Continues `run` on `connection` once its task settled the stop: right after the stop is recorded its task may still
/// be ending, and the runner refuses a run it still executes.
async fn continue_on(
  scheduler: &orchester_runner::execution::Scheduler,
  run: orchester_core::id::RunId,
  connection: orchester_core::id::ConnectionId,
) {
  for _ in 0..50 {
    match scheduler.continue_on_connection(run, connection).await {
      Ok(()) => return,
      Err(Rejection::RunNotPaused) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
      Err(other) => panic!("the step does not continue: {other:?}"),
    }
  }
  panic!("the run's task never settled its stop");
}

fn stopped_at_plan_limit(run: &orchester_core::run::Run) -> bool {
  matches!(
    run.status(),
    RunStatus::Paused(PauseReason::LimitReached(RunLimit::PlanLimit { .. }))
  )
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_a_run_stops_at_its_accounts_plan_limit_and_waits_for_the_same_session() {
  let (project, work, _personal, _) = at_the_limit();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, id, stopped_at_plan_limit).await;
  let db = ProjectDb::open(root).expect("open");
  let reading = db.last_limit_reading(id).expect("read").expect("the refusal's reading");
  assert!(
    reading.resets_at(Timestamp::now()).is_some(),
    "the run knows when to continue"
  );
  std::fs::write(work.path().join("plan-room"), "").expect("the window reset");
  scheduler.resume_run(id).await.expect("the run continues");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let argv = reported(&ProjectDb::open(root).expect("open"), id, "argv: ");
  assert!(
    argv.last().is_some_and(|argv| argv.contains("--resume")),
    "the same session continues: {argv:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_a_person_continues_a_stopped_step_on_another_account_as_a_new_session() {
  let (project, _work, personal, other) = at_the_limit();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, id, stopped_at_plan_limit).await;
  continue_on(&scheduler, id, other).await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let argv = reported(&db, id, "argv: ");
  assert!(
    argv.last().is_some_and(|argv| !argv.contains("--resume")),
    "a new session: {argv:?}"
  );
  let folders = reported(&db, id, "account folder: ");
  assert_eq!(folders.last(), Some(&personal.path().display().to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_a_run_whose_reset_passed_while_no_runner_ran_continues_when_one_starts() {
  let (project, work, _personal, _) = at_the_limit();
  std::fs::write(work.path().join("plan-reset"), "2").expect("Work's limit resets in two seconds");
  let root = project.repo.path();
  let (first, mut events) = scheduler(root);
  let id = first
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, id, stopped_at_plan_limit).await;
  // The first runner would continue it 30 s after the reset; another starts once the reset passed.
  tokio::time::sleep(std::time::Duration::from_secs(3)).await;
  std::fs::write(work.path().join("plan-room"), "").expect("the window reset");
  let (second, mut events) = scheduler(root);
  second
    .attach(AttachSession::Reconnect, NonZeroU32::new(2).expect("above zero"))
    .await
    .expect("attached");
  let run = tokio::time::timeout(
    std::time::Duration::from_secs(20),
    wait_for(root, &mut events, id, |run| run.status().is_terminal()),
  )
  .await
  .expect("the run continues as soon as the runner starts");
  assert_eq!(run.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_a_reset_timer_continues_only_the_stop_it_waits_for() {
  let (project, work, personal, other) = at_the_limit();
  std::fs::write(work.path().join("plan-reset"), "1").expect("Work's limit resets in a second");
  std::fs::remove_file(personal.path().join("plan-room")).expect("Personal is at its limit too");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, id, stopped_at_plan_limit).await;
  let first_stop = std::time::Instant::now();
  continue_on(&scheduler, id, other).await;
  wait_for(root, &mut events, id, stopped_at_plan_limit).await;
  let invocations = ProjectDb::open(root)
    .expect("open")
    .invocations(id)
    .expect("read")
    .len();
  // Past Work's reset and the 30 s after it, when a timer for the first stop would continue the run.
  tokio::time::sleep(
    (first_stop + std::time::Duration::from_secs(34)).saturating_duration_since(std::time::Instant::now()),
  )
  .await;
  let db = ProjectDb::open(root).expect("open");
  assert!(
    stopped_at_plan_limit(&db.run(id).expect("read").expect("the run")),
    "it waits for Personal's reset"
  );
  assert_eq!(
    db.invocations(id).expect("read").len(),
    invocations,
    "no agent started early"
  );
}

/// A second task of the project, queued on its pipeline as the first is.
fn another_task(root: &Path) -> orchester_core::id::TaskId {
  // As the first task's: the fake Claude hits its account's plan limit unless the account has room.
  task_with_brief(root, "Hit the plan limit")
}

/// A task of the project with `brief`, queued on its pipeline as the first is.
fn task_with_brief(root: &Path, brief: &str) -> orchester_core::id::TaskId {
  use orchester_core::{
    resource::{Pipeline, ResourceKind},
    store::{ResourceStore, TaskStore},
    task::{TaskDetails, TaskLimits},
  };
  let mut db = ProjectDb::open(root).expect("open");
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let version = db
    .latest_version::<Pipeline>(pipeline)
    .expect("read")
    .expect("published")
    .number();
  let details = TaskDetails {
    title: orchester_core::name::Name::new("Say hello again").expect("a title"),
    brief: brief.to_owned(),
    acceptance_criteria: vec!["It says hello".to_owned()],
    pipeline: Some(pipeline),
    limits: TaskLimits::new(None, None).expect("limits"),
  };
  let mut task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
    .expect("task")
    .remove(0);
  task.queue(version).expect("a complete draft queues");
  db.save_task(&task).expect("saved").saved().expect("no conflict").id()
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_a_new_run_waits_for_its_accounts_reset_without_an_agent_and_starts_elsewhere_when_asked() {
  let (project, _work, personal, other) = at_the_limit();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let first = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the first run starts");
  wait_for(root, &mut events, first, stopped_at_plan_limit).await;
  let task = another_task(root);
  let second = scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the second run starts");
  wait_for(root, &mut events, second, stopped_at_plan_limit).await;
  let db = ProjectDb::open(root).expect("open");
  assert!(
    db.invocations(second).expect("read").is_empty(),
    "no agent started on Work while its limit holds"
  );
  assert!(
    scheduler.wait_for_plan_reset(second).is_ok(),
    "it waits for Work's reset"
  );
  continue_on(&scheduler, second, other).await;
  let run = wait_for(root, &mut events, second, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let folders = reported(&ProjectDb::open(root).expect("open"), second, "account folder: ");
  assert_eq!(folders.last(), Some(&personal.path().display().to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_a_person_tries_a_waiting_run_on_its_own_account_now() {
  let (project, work, _personal, _) = at_the_limit();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let first = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the first run starts");
  wait_for(root, &mut events, first, stopped_at_plan_limit).await;
  let second = scheduler
    .start_run(IdempotencyKey::generate(), another_task(root))
    .await
    .expect("the second run starts");
  wait_for(root, &mut events, second, stopped_at_plan_limit).await;
  let started = |root: &Path| {
    ProjectDb::open(root)
      .expect("open")
      .invocations(second)
      .expect("read")
      .len()
  };
  assert_eq!(started(root), 0, "it waits without an agent");
  try_now(&scheduler, second).await;
  wait_for(root, &mut events, second, |run| {
    stopped_at_plan_limit(run) && started(root) == 1
  })
  .await;
  std::fs::write(work.path().join("plan-room"), "").expect("Work's limit lifted early");
  try_now(&scheduler, second).await;
  let run = wait_for(root, &mut events, second, |run| run.status().is_terminal()).await;
  assert_eq!(
    run.status(),
    &RunStatus::Succeeded,
    "tried on Work again, now with room"
  );
}

/// A person's *Try now* holds until the step's agent has a slot: with every slot taken when the run continues, the
/// step waits for one and then asks the CLI, rather than waiting for the reset again without asking.
#[tokio::test(flavor = "multi_thread")]
async fn accounts_a_try_now_waits_for_a_free_agent_slot_and_still_asks_the_cli() {
  let (project, _work, _personal, _) = at_the_limit();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let slots = |count: u32| NonZeroU32::new(count).expect("above zero");
  scheduler
    .attach(
      AttachSession::Fresh {
        previous_end: orchester_ipc::SessionEnd::Clean,
      },
      slots(2),
    )
    .await
    .expect("attached");
  // An agent that keeps working on Work, started before Work reached its limit.
  let busy = scheduler
    .start_run(IdempotencyKey::generate(), task_with_brief(root, "Keep working."))
    .await
    .expect("the busy run starts");
  let invoked = |root: &Path, run| {
    ProjectDb::open(root)
      .expect("open")
      .invocations(run)
      .expect("read")
      .len()
  };
  wait_for(root, &mut events, busy, |_| invoked(root, busy) == 1).await;
  let first = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the first run starts");
  wait_for(root, &mut events, first, stopped_at_plan_limit).await;
  let second = scheduler
    .start_run(IdempotencyKey::generate(), another_task(root))
    .await
    .expect("the second run starts");
  wait_for(root, &mut events, second, stopped_at_plan_limit).await;
  try_now(&scheduler, second).await;
  // Every slot is taken now: the busy run's agent holds the only one.
  scheduler.set_agent_slots(slots(1)).await.expect("lowered");
  scheduler.cancel_run(busy).await.expect("cancelled");
  wait_for(root, &mut events, second, |run| {
    stopped_at_plan_limit(run) && invoked(root, second) == 1
  })
  .await;
}

/// Tries `run` on its own account now once its task settled the stop.
async fn try_now(scheduler: &orchester_runner::execution::Scheduler, run: orchester_core::id::RunId) {
  for _ in 0..50 {
    match scheduler.try_at_plan_limit(run).await {
      Ok(()) => return,
      Err(Rejection::RunNotPaused) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
      Err(other) => panic!("the run is not tried: {other:?}"),
    }
  }
  panic!("the run's task never settled its stop");
}
