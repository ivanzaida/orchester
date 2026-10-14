//! `start_qa_app` end to end against `orchester-fake-codex`: the runner starts a QA copy through the worktree's
//! `tools/qa-app.ps1`, keeps it for the invocation so `call_run_app` without a pid reaches it, and stops it when the
//! agent's turn ends. One QA slot: a second run's start waits, noted in its transcript, until the first app stops.
//!
//! The stand-in `tools/qa-app.ps1` copies `orchester-test-command` into `.tmp/qa/<name>/bin/orchester-app.exe` and
//! starts it as a stand-in app with its MCP discovery file. It is a Windows PowerShell script; on macOS the runner runs
//! the script with `pwsh`, which is not part of the test setup, so these tests are Windows only.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test qa_app qa_app`.
#![cfg(windows)]

mod support;

use std::{fs, path::Path};

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::{IdempotencyKey, RunId},
  run::{Answerer, ApprovalAnswer, RunStatus},
  store::ActivityStore,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{
  PATIENCE, Setup, StepSpec, git, is_running, pending_approval, project, queue_another_task, reported, scheduler,
  wait_for,
};

const TEST_COMMAND: &str = env!("CARGO_BIN_EXE_orchester-test-command");

/// The stand-in `tools/qa-app.ps1`: the real script's arguments and output, a stand-in app instead of a build.
fn stand_in_script() -> String {
  format!(
    r#"param([string]$Name, [string]$Scenario, [switch]$FakeAgents, [switch]$NoRunner)
$ErrorActionPreference = "Stop"
$qa = Join-Path (Split-Path -Parent $PSScriptRoot) ".tmp/qa/$Name"
$bin = Join-Path $qa "bin"
$mcp = Join-Path $qa "mcp"
New-Item -ItemType Directory -Force -Path $bin, $mcp | Out-Null
$app = Join-Path $bin "orchester-app.exe"
Copy-Item -LiteralPath '{TEST_COMMAND}' -Destination $app -Force
$started = Start-Process -FilePath $app -ArgumentList "serve-app `"$mcp`"" -WindowStyle Hidden -PassThru
$discovery = Join-Path $mcp "$($started.Id).json"
$deadline = (Get-Date).AddSeconds(20)
while (-not (Test-Path $discovery) -and (Get-Date) -lt $deadline) {{ Start-Sleep -Milliseconds 100 }}
Write-Output "scenario $Scenario"
Write-Output "pid $($started.Id)"
Write-Output "mcp $mcp"
Write-Output "discovery $discovery"
"#
  )
}

/// A project whose one step starts the QA app (and holds it while `fake-hold` exists), with the stand-in script
/// committed and `.tmp/` ignored as in Orchester's own repository.
fn qa_project() -> support::TestProject {
  let project = project(&Setup {
    steps: vec![StepSpec::new(
      "verify",
      "Verify",
      "Start the QA app. Hold the QA app.",
      &[],
    )],
    ..Setup::new("Verify the screen.")
  });
  let root = project.repo.path();
  fs::create_dir_all(root.join("tools")).expect("tools");
  fs::write(root.join("tools").join("qa-app.ps1"), stand_in_script()).expect("script");
  fs::write(root.join(".gitignore"), ".tmp/\n").expect("ignore");
  git(root, &["add", "tools/qa-app.ps1", ".gitignore"]);
  git(root, &["commit", "--quiet", "-m", "QA script"]);
  project
}

/// Approves the run's `start_qa_app`, which asks: it builds and runs the worktree's code outside the sandbox.
async fn approve_start(scheduler: &Scheduler, root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) {
  let request = pending_approval(root, events, run).await;
  scheduler
    .decide_tool_approval(run, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
}

/// The answers of the run's Orchester calls, `orchester-call: <tool> <ok|error>: <first line>`.
fn calls(root: &Path, run: RunId) -> Vec<String> {
  reported(&ProjectDb::open(root).expect("open"), run, "orchester-call: ")
}

/// The pid `start_qa_app` answered, once the run's call returned.
async fn started_pid(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) -> String {
  tokio::time::timeout(PATIENCE, async {
    loop {
      if let Some(call) = calls(root, run).iter().find(|call| call.starts_with("start_qa_app")) {
        assert!(call.starts_with("start_qa_app ok"), "{call}");
        let pid = call
          .split("pid ")
          .nth(1)
          .expect("a pid")
          .split('.')
          .next()
          .expect("digits");
        return pid.to_owned();
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("the QA app starts")
}

/// Waits until the run's transcript says its start waits for the QA slot.
async fn waits_for_slot(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) {
  let noted = |db: &ProjectDb| {
    db.activity(run, 0).expect("activity").iter().any(|entry| {
      entry.event
        == AgentEvent::RunnerProblem {
          problem: RunnerProblem::WaitingForQaSlot,
        }
    })
  };
  tokio::time::timeout(PATIENCE, async {
    while !noted(&ProjectDb::open(root).expect("open")) {
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("the start waits for the QA slot");
}

#[tokio::test(flavor = "multi_thread")]
async fn qa_app_one_slot_second_run_waits_then_gets_it_and_apps_stop_at_turn_end() {
  let project = qa_project();
  let root = project.repo.path();
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let second_task = queue_another_task(root);
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .set_agent_slots(std::num::NonZeroU32::new(2).expect("two"))
    .await
    .expect("two agents at once");

  let first = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("first run");
  approve_start(&scheduler, root, &mut events, first).await;
  let first_app = started_pid(root, &mut events, first).await;
  assert!(is_running(&first_app), "the first run's QA app runs");
  assert!(
    calls(root, first)
      .iter()
      .any(|call| call == "call_run_app ok: app ready"),
    "call_run_app without a pid reaches the step's QA app: {:?}",
    calls(root, first)
  );

  let second = scheduler
    .start_run(IdempotencyKey::generate(), second_task)
    .await
    .expect("second run");
  approve_start(&scheduler, root, &mut events, second).await;
  waits_for_slot(root, &mut events, second).await;
  assert!(
    !calls(root, second).iter().any(|call| call.starts_with("start_qa_app")),
    "the second start waits while the first app holds the slot"
  );

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, first, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let second_app = started_pid(root, &mut events, second).await;
  assert!(!is_running(&first_app), "the first app stopped when its turn ended");
  let run = wait_for(root, &mut events, second, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let answered = calls(root, second);
  assert!(
    answered
      .iter()
      .any(|call| call.starts_with("start_qa_app ok") && call.contains("waited")),
    "{answered:?}"
  );
  support::wait_for_idle(&scheduler).await;
  assert!(!is_running(&second_app), "the second app stopped when its turn ended");
}
