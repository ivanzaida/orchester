//! Projects whose steps start a QA app through `start_qa_app`, for the QA app tests: a stand-in `tools/qa-app.ps1`
//! committed in the repository, which copies `orchester-test-command` into `.tmp/qa/<name>/bin/orchester-app.exe` and
//! starts it as a stand-in app with its MCP discovery file. It writes the app's pid to `<repo>/.orchester/qa-report/
//! <name>.app` and its own to `<name>.script`. By scenario it prints the real script's `pid` and `discovery` lines
//! (any), never ends (`hang`), ends without naming the app (`nopid`), or leaves a program that holds its output open
//! (`holdpipe`, its pid in `<name>.holder`). It is a Windows PowerShell script.

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::RunId,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  run::{Answerer, ApprovalAnswer},
  store::ActivityStore,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use super::{PATIENCE, Setup, StepSpec, TestProject, git, is_running, pending_approval, project, reported};

const TEST_COMMAND: &str = env!("CARGO_BIN_EXE_orchester-test-command");

/// Where the stand-in script reports the pids it started.
pub fn report_folder(root: &Path) -> PathBuf {
  root.join(".orchester").join("qa-report")
}

fn stand_in_script(report: &Path) -> String {
  let report = report.display();
  format!(
    r#"param([string]$Name, [string]$Scenario, [switch]$FakeAgents, [switch]$NoRunner)
$ErrorActionPreference = "Stop"
$qa = Join-Path (Split-Path -Parent $PSScriptRoot) ".tmp/qa/$Name"
$bin = Join-Path $qa "bin"
$mcp = Join-Path $qa "mcp"
$report = '{report}'
New-Item -ItemType Directory -Force -Path $bin, $mcp, $report | Out-Null
Set-Content -Path (Join-Path $report "$Name.script") -Value $PID
$app = Join-Path $bin "orchester-app.exe"
Copy-Item -LiteralPath '{TEST_COMMAND}' -Destination $app -Force
$started = Start-Process -FilePath $app -ArgumentList "serve-app `"$mcp`"" -WindowStyle Hidden -PassThru
Set-Content -Path (Join-Path $report "$Name.app") -Value $started.Id
$discovery = Join-Path $mcp "$($started.Id).json"
$deadline = (Get-Date).AddSeconds(20)
while (-not (Test-Path $discovery) -and (Get-Date) -lt $deadline) {{ Start-Sleep -Milliseconds 100 }}
if ($Scenario -eq "hang") {{ Start-Sleep -Seconds 600 }}
if ($Scenario -eq "holdpipe") {{
  $holder = Start-Process -FilePath $app -ArgumentList "sleep 600" -NoNewWindow -PassThru
  Set-Content -Path (Join-Path $report "$Name.holder") -Value $holder.Id
}}
if ($Scenario -eq "nopid") {{ Write-Output "started"; exit 0 }}
Write-Output "scenario $Scenario"
Write-Output "pid $($started.Id)"
Write-Output "mcp $mcp"
Write-Output "discovery $discovery"
"#
  )
}

/// A project of `steps` whose agents write the worktree and reach the network, as `start_qa_app` needs, with the
/// stand-in script committed and `.tmp/` ignored as in Orchester's own repository.
pub fn qa_project(steps: Vec<StepSpec>) -> TestProject {
  let project = project(&Setup {
    steps,
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::AnyInWorktree,
      NetworkAccess::On,
    )),
    ..Setup::new("Verify the screen.")
  });
  let root = project.repo.path();
  fs::create_dir_all(root.join("tools")).expect("tools");
  fs::write(
    root.join("tools").join("qa-app.ps1"),
    stand_in_script(&report_folder(root)),
  )
  .expect("script");
  fs::write(root.join(".gitignore"), ".tmp/\n").expect("ignore");
  git(root, &["add", "tools/qa-app.ps1", ".gitignore"]);
  git(root, &["commit", "--quiet", "-m", "QA script"]);
  project
}

/// One agent step, Verify, with `instructions` for the fake CLI.
pub fn verify_step(instructions: &'static str) -> Vec<StepSpec> {
  vec![StepSpec::new("verify", "Verify", instructions, &[])]
}

/// Approves the run's first waiting request, as the person does.
pub async fn approve(scheduler: &Scheduler, root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) {
  let request = pending_approval(root, events, run).await;
  scheduler
    .decide_tool_approval(run, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
}

/// The answers of the run's Orchester calls, `orchester-call: <tool> <ok|error>: <first line>`.
pub fn calls(root: &Path, run: RunId) -> Vec<String> {
  reported(&ProjectDb::open(root).expect("open"), run, "orchester-call: ")
}

/// The pid `start_qa_app` answered, once the run's call returned.
pub async fn started_pid(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) -> String {
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

/// The pid the stand-in script reported in `<name>.<what>`, once it did.
pub async fn reported_pid(root: &Path, name: &str, what: &str) -> String {
  let file = report_folder(root).join(format!("{name}.{what}"));
  tokio::time::timeout(PATIENCE, async {
    loop {
      if let Ok(pid) = fs::read_to_string(&file)
        && !pid.trim().is_empty()
      {
        return pid.trim().to_owned();
      }
      tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
  })
  .await
  .expect("the script reports its pid")
}

/// Waits until process `pid` no longer runs.
pub async fn gone(pid: &str, what: &str) {
  let waited = tokio::time::timeout(PATIENCE, async {
    while is_running(pid) {
      tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
  })
  .await;
  assert!(waited.is_ok(), "{what} (pid {pid}) still runs");
}

/// Waits until the run's transcript holds `problem`.
pub async fn noted(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId, problem: RunnerProblem) {
  let note = AgentEvent::RunnerProblem { problem };
  let holds = |db: &ProjectDb| {
    db.activity(run, 0)
      .expect("activity")
      .iter()
      .any(|entry| entry.event == note)
  };
  tokio::time::timeout(PATIENCE, async {
    while !holds(&ProjectDb::open(root).expect("open")) {
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("the note is in the transcript");
}
