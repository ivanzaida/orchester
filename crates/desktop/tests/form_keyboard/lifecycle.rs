//! Run lifecycle end to end, headlessly, with the real runner on `orchester-fake-codex`
//! (`docs/architecture/storage.md`, "Lifecycle"): the work a close would interrupt, the status
//! bar while the runner drops and reconnects, the start-up recovery after a crash, continuing an
//! interrupted run from its task page, and the daily project budget. The runner talks to the
//! desktop over a named pipe, which exists on Windows only.
#![cfg(windows)]

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
  time::Duration,
};

use orchester_core::{
  limits::ProjectLimits,
  run::{InterruptCause, PauseReason, RunLimit, RunStatus},
  store::{ActivityStore, SettingsStore},
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
  task_run::{
    create_and_run, fill_new_task, has, init_repository, latest_run, programs, run_finished, seed, wait_long,
  },
};

/// A desktop whose project runs Implement on the fake CLI, with the runner program `runner`.
fn desktop(runner: PathBuf, fake: &Path) -> Desktop {
  Desktop::with_runner(runner, init_repository, |repo| seed(repo, fake, false))
}

fn text_of(desktop: &mut Desktop, id: &str, text: &str) -> bool {
  desktop.tree.get_element_by_id(id).is_some() && shows(desktop, id, text)
}

/// The runner processes of project `repo` that this test process started. Tests run side by
/// side in one process, so the project path tells their runners apart.
fn own_runners(repo: &Path) -> Vec<String> {
  let filter = format!("ParentProcessId={} AND Name='orchester-runner.exe'", std::process::id());
  let script = format!(
    "Get-CimInstance Win32_Process -Filter \"{filter}\" | Where-Object {{ $_.CommandLine -like '*{}*' }} |      ForEach-Object {{ $_.ProcessId }}",
    repo.display()
  );
  let output = Command::new("powershell")
    .args(["-NoProfile", "-Command", &script])
    .output()
    .expect("ask Windows for the runner processes");
  String::from_utf8_lossy(&output.stdout)
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .map(str::to_owned)
    .collect()
}

/// Kills the runner of project `repo`, as a crash of the runner would end it.
fn kill_runner_of(repo: &Path) {
  let runners = own_runners(repo);
  assert_eq!(runners.len(), 1, "one runner of this test's project runs: {runners:?}");
  let status = Command::new("taskkill")
    .args(["/PID", &runners[0], "/F"])
    .status()
    .expect("run taskkill");
  assert!(status.success(), "taskkill {}", runners[0]);
}

/// Removes `file` once the process that ran it has let go of it.
fn remove_when_free(file: &Path) {
  for _ in 0..100 {
    if fs::remove_file(file).is_ok() {
      return;
    }
    std::thread::sleep(Duration::from_millis(50));
  }
  panic!("{} stays in use", file.display());
}

/// Whether the agent of the latest run reported its CLI session, which continuing resumes.
fn session_recorded(repo: &Path) -> bool {
  let Some(run) = latest_run(repo) else {
    return false;
  };
  ProjectDb::open(repo)
    .expect("open")
    .invocations(run.id())
    .expect("invocations")
    .iter()
    .any(|invocation| invocation.session.is_some())
}

#[test]
fn lifecycle_active_work_and_agents_count_follow_the_runner() {
  let (runner, fake) = programs();
  let mut desktop = desktop(runner, &fake);
  create_and_run(&mut desktop, "Keep working.", "1400000");
  wait_long(&mut desktop, "the agent at work", |desktop| {
    !desktop.runners.active_work().is_empty()
      && text_of(desktop, "status-agents", "1/4")
      && has(desktop, "task-cancel-run")
  });
  let work = desktop.runners.active_work();
  assert_eq!(work.len(), 1);
  assert_eq!(work[0].project(), "Kontur");
  assert_eq!(work[0].task_key(), "KON-1");
  assert_eq!(work[0].task_title(), "Say hello");
  assert_eq!(work[0].step(), "Implement");
  assert_eq!(work[0].agent(), "Builder");
  assert_eq!(work[0].model(), "gpt-5.5");
  desktop.click("task-cancel-run");
  wait_long(&mut desktop, "no agent at work", |desktop| {
    desktop.runners.active_work().is_empty() && text_of(desktop, "status-agents", "0/4")
  });
  // The cancelled agent was stopped; it did not finish.
  wait_long(&mut desktop, "the latest activity", |desktop| {
    text_of(desktop, "task-latest", "Implement · Builder stopped")
  });
  assert!(!text_of(&mut desktop, "task-latest", "finished"));
}

#[test]
fn lifecycle_a_dropped_runner_shows_the_interrupted_runs_then_continues_them() {
  let (runner, fake) = programs();
  // A copy of the runner, so it can be taken away while the desktop reconnects.
  let copies = tempfile::tempdir().expect("temporary directory");
  let copy = copies.path().join(runner.file_name().expect("runner file name"));
  fs::copy(&runner, &copy).expect("copy the runner");
  let mut desktop = desktop(copy.clone(), &fake);
  create_and_run(&mut desktop, "Keep working.", "1400000");
  wait_long(&mut desktop, "the agent at work", |desktop| {
    text_of(desktop, "status-agents", "1/4") && session_recorded(desktop.repo())
  });

  kill_runner_of(desktop.repo());
  remove_when_free(&copy);
  wait_long(&mut desktop, "the runner offline (G)", |desktop| {
    text_of(desktop, "status-runner", "Runner offline")
  });
  assert!(text_of(
    &mut desktop,
    "status-activity",
    "1 run interrupted · nothing runs until the runner starts"
  ));
  assert!(has(&mut desktop, "status-restart-runner"));
  assert!(text_of(&mut desktop, "status-agents", "—"));

  fs::copy(&runner, &copy).expect("bring the runner back");
  desktop.click("status-restart-runner");
  wait_long(&mut desktop, "the reconnect event (F)", |desktop| {
    text_of(
      desktop,
      "status-activity",
      "Runner reconnected · 1 interrupted run continued",
    )
  });
  wait_long(&mut desktop, "the run to finish", |desktop| {
    run_finished(desktop.repo())
  });
  let run = latest_run(desktop.repo()).expect("run");
  let db = ProjectDb::open(desktop.repo()).expect("open");
  let kinds: Vec<String> = orchester_core::store::RunStore::events(&db, run.id())
    .expect("events")
    .iter()
    .map(|event| event.kind().name().to_owned())
    .collect();
  assert!(kinds.contains(&"interrupted".to_owned()), "{kinds:?}");
  assert!(kinds.contains(&"continued".to_owned()), "{kinds:?}");
}

#[test]
fn lifecycle_a_reconnecting_runner_names_the_interrupted_runs() {
  let (runner, fake) = programs();
  let mut desktop = desktop(runner, &fake);
  create_and_run(&mut desktop, "Keep working.", "1400000");
  wait_long(&mut desktop, "the agent at work", |desktop| {
    text_of(desktop, "status-agents", "1/4") && session_recorded(desktop.repo())
  });
  kill_runner_of(desktop.repo());
  // E: reconnecting; it lasts for the first backoff and the next start.
  wait_long(&mut desktop, "the reconnecting runner (E)", |desktop| {
    text_of(desktop, "status-runner", "Reconnecting to runner")
      && text_of(
        desktop,
        "status-activity",
        "1 run interrupted · it continues from its last checkpoint",
      )
  });
  wait_long(&mut desktop, "the run to finish", |desktop| {
    run_finished(desktop.repo())
  });
}

#[test]
fn lifecycle_after_a_crash_the_recovery_continues_all_runs() {
  let (runner, fake) = programs();
  let mut desktop = desktop(runner, &fake);
  create_and_run(&mut desktop, "Keep working.", "1400000");
  wait_long(&mut desktop, "the agent at work", |desktop| {
    text_of(desktop, "status-agents", "1/4") && session_recorded(desktop.repo())
  });
  let mut desktop = desktop.crash_and_restart();
  wait_long(&mut desktop, "the recovery", |desktop| has(desktop, "recovery-dialog"));
  for text in [
    "The last session ended before agents could save",
    "Orchester stopped unexpectedly and 1 run was interrupted.",
    "KON-1",
    "Say hello",
    "Kontur · Run #1",
    "Implement",
    "Builder · Codex · gpt-5.5",
    "Orchester stopped unexpectedly",
    "No checkpoint yet",
    "No uncommitted changes",
    "Continue from checkpoint",
  ] {
    assert!(shows(&mut desktop, "recovery-dialog", text), "{text}");
  }
  desktop.click("recovery-continue-all");
  wait_long(&mut desktop, "the run to finish", |desktop| {
    run_finished(desktop.repo())
  });
  wait_long(&mut desktop, "the recovery to close", |desktop| {
    !has(desktop, "recovery-dialog")
  });
}

#[test]
fn lifecycle_runs_not_continued_wait_until_their_task_page_continues_them() {
  let (runner, fake) = programs();
  let mut desktop = desktop(runner, &fake);
  create_and_run(&mut desktop, "Keep working.", "1400000");
  wait_long(&mut desktop, "the agent at work", |desktop| {
    text_of(desktop, "status-agents", "1/4") && session_recorded(desktop.repo())
  });
  let mut desktop = desktop.crash_and_restart();
  wait_long(&mut desktop, "the recovery", |desktop| has(desktop, "recovery-dialog"));
  desktop.click("recovery-dont-continue");
  wait_long(&mut desktop, "the recovery to close", |desktop| {
    !has(desktop, "recovery-dialog")
  });
  // The project's runner records the interruption when it attaches for the new session.
  wait_long(&mut desktop, "the interrupted run", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status() == &RunStatus::Interrupted(InterruptCause::Crashed))
  });
  desktop.click("nav-tasks");
  wait_long(&mut desktop, "the task row", |desktop| {
    text_of(desktop, "task-row-KON-1", "Interrupted") && text_of(desktop, "task-row-KON-1", "No checkpoint yet")
  });
  desktop.click("task-row-KON-1");
  wait_long(&mut desktop, "Continue run", |desktop| {
    has(desktop, "task-continue-run")
  });
  desktop.click("task-continue-run");
  wait_long(&mut desktop, "the run to finish", |desktop| {
    run_finished(desktop.repo())
  });
}

#[test]
fn lifecycle_the_daily_budget_stops_the_run_and_holds_back_new_work() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed(repo, &fake, false);
    let tight = ProjectLimits::new(
      TokenBudget::new(10_000_000).expect("above zero"),
      TokenBudget::new(1_400_000).expect("above zero"),
      TokenBudget::new(5_000).expect("above zero"),
      Duration::from_secs(30 * 60),
    )
    .expect("limits");
    ProjectDb::open(repo)
      .expect("open")
      .save_project_limits(&tight)
      .expect("save limits");
  });
  create_and_run(&mut desktop, "Work in turns.", "1400000");
  wait_long(&mut desktop, "the daily budget stop", |desktop| {
    latest_run(desktop.repo())
      .is_some_and(|run| run.status() == &RunStatus::Paused(PauseReason::LimitReached(RunLimit::DailyBudget)))
      && has(desktop, "task-resume-run")
  });
  wait_long(&mut desktop, "the notice", |desktop| {
    text_of(desktop, "task-notice", "Stopped at the daily project budget")
  });
  assert!(!has(&mut desktop, "raise-limit"));
  desktop.click("task-resume-run");
  let refusal = "Daily token budget used — new runs wait until tomorrow or raise it in Project settings.";
  wait_long(&mut desktop, "the refusal", |desktop| {
    text_of(desktop, "task-page", refusal)
  });
  // New work waits too: Create & run queues the task, and the runner refuses its run.
  desktop.click("task-breadcrumb");
  fill_new_task(&mut desktop, "Append one line to README.md.", "1400000");
  desktop.click("create-and-run");
  wait_long(&mut desktop, "the refusal in New task", |desktop| {
    desktop.tree.root().is_some_and(|root| contains_text(root, refusal)) && has(desktop, "create-and-run")
  });
}
