//! The permission prompt relay and its runner end: only the relay with the invocation's token is
//! served, a relay that leaves while a person decides ends the wait, and a relay that cannot reach
//! Orchester denies as unavailable and is reported. Needs `git`. Run with `cargo test -p
//! orchester-runner --features test-agent --test claude_prompt_relay claude_prompt_relay`.

mod support;

use std::{
  io::{BufRead, BufReader, Write},
  path::Path,
  process::{Command, Stdio},
};

use orchester_core::{
  connection::{
    AgentProvider,
    stream::{AgentEvent, RunnerProblem},
  },
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::StepKey,
  run::{Answerer, ApprovalAnswer, PendingToolApproval, RunStatus, StepFailure, Stuck},
  store::ActivityStore,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, reported, scheduler_with, settled, wait_for};

const RELAY: &str = env!("CARGO_BIN_EXE_orchester-runner");

fn asking(brief: &'static str) -> TestProject {
  project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::AskBefore,
      NetworkAccess::Off,
    )),
    ..Setup::new(brief)
  })
}

async fn pending(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) -> PendingToolApproval {
  tokio::time::timeout(PATIENCE, async {
    loop {
      if let Some(request) = ProjectDb::open(root)
        .expect("open")
        .pending_tool_approvals(run)
        .expect("requests")
        .first()
      {
        return request.clone();
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("an approval arrives")
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_prompt_relay_serves_only_the_one_relay_with_the_token() {
  let project = asking("Ask before publishing. Knock first.");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending(root, &mut events, id).await;
  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(reported(&db, id, "knock: "), ["closed"], "a wrong token is not served");
  assert_eq!(reported(&db, id, "permission: "), ["toolu_publish allow"]);
  assert_eq!(
    reported(&db, id, "second: "),
    ["closed"],
    "no second connection is taken, even with the token"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_prompt_relay_leaving_while_a_person_decides_interrupts_the_request() {
  let project = asking("Ask before publishing. Leave while waiting.");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let request = pending(root, &mut events, id).await;
  std::fs::write(root.join(".orchester").join("fake-release"), "").expect("release");
  // The turn broke without a result: its step restarts as a fresh session.
  wait_for(root, &mut events, id, |run| run.attempts().len() == 2).await;
  let db = ProjectDb::open(root).expect("open");
  assert!(
    db.pending_tool_approvals(id)
      .expect("requests")
      .iter()
      .all(|pending| pending.id != request.id)
  );
  assert!(db.has_interrupted_tool_approval(id).expect("approvals"));
  assert_eq!(
    scheduler
      .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
      .await,
    Err(Rejection::ToolApprovalNotAwaited),
    "a later decision is not recorded as delivered"
  );
  scheduler.cancel_run(id).await.expect("cancelled");
}

/// Runs `brief` on a step that asks before commands to its end; its run and runner problems.
async fn finished(brief: &'static str) -> (orchester_core::run::Run, Vec<RunnerProblem>) {
  let project = asking(brief);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, settled).await;
  let problems = ProjectDb::open(root)
    .expect("open")
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::RunnerProblem { problem } => Some(problem),
      _ => None,
    })
    .collect();
  (run, problems)
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_prompt_relay_that_never_reached_the_runner_fails_the_step() {
  // The fake CLI starts its session and works without ever starting the prompt server.
  let (run, problems) = finished("Leave the prompt unstarted. Append one line.").await;
  // The step restarts once and its prompt again never reaches the runner.
  assert_eq!(
    problems,
    [
      RunnerProblem::PermissionPromptUnreachable,
      RunnerProblem::PermissionPromptUnreachable
    ]
  );
  assert!(
    matches!(run.status(), RunStatus::WaitingForPerson(Stuck::BrokeAgain(_))),
    "{:?}",
    run.status()
  );
  let implement = run
    .latest_execution(&StepKey::new("implement").expect("key"))
    .expect("the step ran");
  assert_eq!(implement.failure(), Some(StepFailure::AgentFailed));
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_prompt_relay_is_not_blamed_when_the_cli_failed_first() {
  let (_, not_signed_in) = finished("Please sign in.").await;
  assert!(!not_signed_in.contains(&RunnerProblem::PermissionPromptUnreachable));
  let (_, refused) = finished("Refuse the model.").await;
  assert!(
    !refused.contains(&RunnerProblem::PermissionPromptUnreachable),
    "{refused:?}"
  );
  assert!(
    refused
      .iter()
      .any(|problem| matches!(problem, RunnerProblem::CliTooOld { .. }))
  );
}

#[test]
fn claude_prompt_relay_without_orchester_denies_as_unavailable_and_says_why() {
  let mut relay = Command::new(RELAY)
    .arg("--claude-permission-prompt")
    .env_remove("ORCHESTER_APPROVAL_ADDRESS")
    .env_remove("ORCHESTER_APPROVAL_TOKEN")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("the relay starts");
  let mut input = relay.stdin.take().expect("stdin");
  let call = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"approve","arguments":{"tool_name":"Bash","input":{"command":"ls"},"tool_use_id":"toolu_1"}}}"#;
  writeln!(input, "{call}").expect("the call");
  drop(input);
  let mut answer = String::new();
  BufReader::new(relay.stdout.take().expect("stdout"))
    .read_line(&mut answer)
    .expect("an answer");
  let output = relay.wait_with_output().expect("the relay ends");
  let answer: serde_json::Value = serde_json::from_str(&answer).expect("JSON");
  let decision: serde_json::Value =
    serde_json::from_str(answer["result"]["content"][0]["text"].as_str().expect("text")).expect("JSON");
  assert_eq!(decision["behavior"], "deny");
  assert!(
    decision["message"]
      .as_str()
      .is_some_and(|message| message.contains("unavailable")),
    "{decision}"
  );
  let log = String::from_utf8_lossy(&output.stderr);
  assert!(log.contains("cannot reach Orchester"), "{log}");
}
