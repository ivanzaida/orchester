//! Shared profile CLI requirements survive storage and are checked before start, resume and retry.

use orchester_core::{
  connection::{AgentConnection, AgentProvider, stream::RunnerProblem},
  run::StuckDecision,
};

use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn team_inputs_connection_kind_mismatch_is_refused_before_a_run() {
  let team = team(&Setup::new("Append one line."));
  let root = team.root();
  let head = git(root, &["rev-parse", "HEAD"]);
  let operation = team.accept_with_cli(team.connection(), &head, Some(AgentProvider::ClaudeCodeCli));
  let (scheduler, _events) = scheduler(root);
  let refused = scheduler.start_accepted_run(operation).await;
  assert_eq!(
    refused,
    Err(Rejection::Preflight {
      problems: ["implement", "review"]
        .map(|step| {
          PreflightProblem::ConnectionMissing {
            step: StepKey::new(step).expect("key"),
          }
        })
        .to_vec()
    })
  );
  let db = ProjectDb::open(root).expect("database");
  assert!(db.runs_of_task(team.task).expect("runs").is_empty());
  assert_eq!(db.run_of_operation(operation).expect("operation"), None);
}

fn change_provider(team: &Team) {
  rusqlite::Connection::open(team.root().join(".orchester/orchester.db"))
    .expect("database")
    .execute(
      "UPDATE connections SET provider = 'claude_code_cli' WHERE id = ?1",
      [team.connection().to_string()],
    )
    .expect("simulate a changed provider");
}

fn override_connection(team: &Team, run: RunId) {
  let connection = AgentConnection::new(
    ConnectionId::generate(),
    name("Other CLI"),
    AgentProvider::ClaudeCodeCli,
    std::path::PathBuf::from(support::FAKE_CLAUDE),
    Timestamp::now(),
  )
  .expect("absolute path")
  .into_connection();
  let mut db = ProjectDb::open(team.root()).expect("database");
  db.create_connection(&connection).expect("connection");
  db.set_step_connection(run, &StepKey::new("review").expect("key"), connection.id())
    .expect("override");
}

#[tokio::test(flavor = "multi_thread")]
async fn team_inputs_connection_kind_is_checked_on_resume_and_every_retry() {
  for override_chosen in [false, true] {
    let team = team(&Setup::new("Work slowly."));
    let root = team.root();
    let head = git(root, &["rev-parse", "HEAD"]);
    let operation = team.accept_with_cli(team.connection(), &head, Some(AgentProvider::CodexCli));
    let run = start_and_pause(&team, operation).await;
    let db = ProjectDb::open(root).expect("database");
    assert_eq!(
      db.run_inputs(run).expect("inputs").expect("kept").bindings().profiles[0].required_cli,
      Some(AgentProvider::CodexCli),
      "required kind survives storage"
    );
    if override_chosen {
      override_connection(&team, run);
    } else {
      change_provider(&team);
    }
    let (scheduler, mut events) = scheduler(root);
    scheduler.resume_run(run).await.expect("resume requested");
    support::qa::noted(
      root,
      &mut events,
      run,
      RunnerProblem::CouldNotStart {
        detail: "the agent profile's connection no longer exists".into(),
      },
    )
    .await;
    let refused = wait_for(root, &mut events, run, |run| {
      matches!(run.status(), RunStatus::WaitingForPerson(_))
    })
    .await;
    wait_for_idle(&scheduler).await;
    let attempts = refused.attempts().len();
    scheduler
      .decide_stuck(run, StuckDecision::Retry)
      .await
      .expect("retry requested");
    wait_for(root, &mut events, run, |run| {
      matches!(run.status(), RunStatus::WaitingForPerson(_)) && run.attempts().len() > attempts
    })
    .await;
    wait_for_idle(&scheduler).await;
    assert_eq!(
      reported(&db, run, "prompt: ").len(),
      1,
      "no mismatched CLI launched on resume or retry"
    );
  }
}
