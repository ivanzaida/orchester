//! Run lifecycle end to end, headlessly, with the real runner on `orchester-fake-codex`
//! (`docs/architecture/storage.md`, "Lifecycle"): the work a close would interrupt, the status
//! bar while the runner drops and reconnects, the start-up recovery after a crash and the changes
//! it links to, continuing an interrupted run from its task page, and the daily project budget.

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
  time::Duration,
};

use orchester_core::{
  connection::{CheckResult, ConnectionCheck, LastCheck},
  limits::ProjectLimits,
  run::{InterruptCause, PauseReason, RunLimit, RunStatus},
  store::{ActivityStore, ConnectionStore, SettingsStore, TaskStore},
  task::TaskStatus,
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_desktop::tasks::changes::run_worktree;
use orchester_store::ProjectDb;

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
  task_run::{
    create_and_run, fill_new_task, has, init_repository, latest_run, programs, run_finished, seed, seed_claude,
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
  let script = format!("Stop-Process -Id {} -Force -ErrorAction Stop", runners[0]);
  powershell(&script, &format!("kill the runner {}", runners[0]));
}

fn powershell(script: &str, what: &str) {
  let output = Command::new("powershell")
    .args(["-NoProfile", "-Command", script])
    .output()
    .unwrap_or_else(|error| panic!("{what}: {error}"));
  assert!(
    output.status.success(),
    "{what}: {}: {}",
    output.status,
    String::from_utf8_lossy(&output.stderr)
  );
}

/// Ends the application as a crash would, then starts it again on the same database.
fn crash_and_restart(desktop: Desktop) -> Desktop {
  desktop.crash_and_restart(|| {})
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
  desktop.wait_until("the agent at work", |desktop| {
    !desktop.runners.active_work().is_empty()
      && text_of(desktop, "status-agents", "1/4")
      && text_of(desktop, "sidebar-runner-state", "Local runner online")
      && text_of(desktop, "sidebar-runner-detail", "1 of 4 agent slots in use")
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
  desktop.wait_until("no agent at work", |desktop| {
    desktop.runners.active_work().is_empty()
      && text_of(desktop, "status-agents", "0/4")
      && text_of(desktop, "sidebar-runner-detail", "0 of 4 agent slots in use")
  });
  // The cancelled agent was stopped; it did not finish.
  desktop.wait_until("the latest activity", |desktop| {
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
  desktop.wait_until("the agent at work", |desktop| {
    text_of(desktop, "status-agents", "1/4") && session_recorded(desktop.repo())
  });

  // Taken away before the kill, so no reconnect attempt can start it again: Windows lets a running
  // program be renamed, not removed.
  fs::rename(&copy, copies.path().join("taken-away.exe")).expect("take the runner away");
  kill_runner_of(desktop.repo());
  desktop.wait_until("the runner offline (G)", |desktop| {
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
  desktop.wait_until("the reconnect event (F)", |desktop| {
    text_of(
      desktop,
      "status-activity",
      "Runner reconnected · 1 interrupted run continued",
    )
  });
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
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
  desktop.wait_until("the agent at work", |desktop| {
    text_of(desktop, "status-agents", "1/4") && session_recorded(desktop.repo())
  });
  kill_runner_of(desktop.repo());
  // E: reconnecting; it lasts for the first backoff and the next start.
  desktop.wait_until("the reconnecting runner (E)", |desktop| {
    text_of(desktop, "status-runner", "Reconnecting to runner")
      && text_of(
        desktop,
        "status-activity",
        "1 run interrupted · it continues from its last checkpoint",
      )
  });
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
}

/// Holds project `repo`'s runner lock (`.orchester/runner.lock`), as the runner of another
/// Orchester app does while that app runs the project.
fn hold_runner_lock(repo: &Path) -> fs::File {
  let lock = fs::OpenOptions::new()
    .write(true)
    .create(true)
    .truncate(false)
    .open(repo.join(".orchester").join("runner.lock"))
    .expect("open the runner lock");
  lock.try_lock().expect("no runner holds the lock yet");
  lock
}

#[test]
fn lifecycle_a_project_another_app_runs_says_so_until_that_app_closes() {
  let (runner, _) = programs();
  let mut other_app = None;
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    other_app = Some(hold_runner_lock(repo));
  });
  desktop.wait_until("the runner notice", |desktop| has(desktop, "runner-notice"));
  for text in [
    "Could not start the runner",
    "Another Orchester window is already running this project's runner. Close that window, then restart the runner.",
  ] {
    assert!(shows(&mut desktop, "runner-notice", text), "{text}");
  }
  assert!(text_of(&mut desktop, "status-runner", "Runner offline"));

  drop(other_app);
  desktop.click("runner-restart");
  desktop.wait_until("the runner online", |desktop| {
    text_of(desktop, "status-runner", "Runner online") && !has(desktop, "runner-notice")
  });
}

#[test]
fn lifecycle_after_a_crash_the_recovery_continues_all_runs() {
  let (runner, fake) = programs();
  let mut desktop = desktop(runner, &fake);
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_until("the agent at work", |desktop| {
    text_of(desktop, "status-agents", "1/4") && session_recorded(desktop.repo())
  });
  let mut desktop = crash_and_restart(desktop);
  desktop.wait_until("the recovery", |desktop| has(desktop, "recovery-dialog"));
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
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  desktop.wait_until("the recovery to close", |desktop| !has(desktop, "recovery-dialog"));
}

#[test]
fn lifecycle_after_a_crash_view_changes_opens_the_changes_not_checkpointed() {
  let (runner, fake) = programs();
  let mut desktop = desktop(runner, &fake);
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_until("the agent at work", |desktop| {
    text_of(desktop, "status-agents", "1/4") && session_recorded(desktop.repo())
  });
  let run = latest_run(desktop.repo()).expect("run").id();
  fs::write(run_worktree(desktop.repo(), run).join("notes.md"), "draft\n").expect("write into the worktree");
  let mut desktop = crash_and_restart(desktop);
  let link = format!("recovery-view-changes-{run}");
  desktop.wait_until("View changes", |desktop| has(desktop, &link));
  assert!(shows(&mut desktop, "recovery-dialog", "not committed"));
  desktop.click(&link);
  desktop.wait_until("the run's changes not checkpointed", |desktop| {
    !has(desktop, "recovery-dialog") && has(desktop, "diff-worktree-file:notes.md")
  });
  // Viewing the changes continues nothing: the run stays interrupted until its page continues it.
  desktop.wait_until("the interrupted run", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status() == &RunStatus::Interrupted(InterruptCause::Crashed))
      && has(desktop, "task-continue-run")
  });
}

#[test]
fn lifecycle_runs_not_continued_wait_until_their_task_page_continues_them() {
  let (runner, fake) = programs();
  let mut desktop = desktop(runner, &fake);
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_until("the agent at work", |desktop| {
    text_of(desktop, "status-agents", "1/4") && session_recorded(desktop.repo())
  });
  let mut desktop = crash_and_restart(desktop);
  desktop.wait_until("the recovery", |desktop| has(desktop, "recovery-dialog"));
  desktop.click("recovery-dont-continue");
  desktop.wait_until("the recovery to close", |desktop| !has(desktop, "recovery-dialog"));
  // The project's runner records the interruption when it attaches for the new session.
  desktop.wait_until("the interrupted run", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status() == &RunStatus::Interrupted(InterruptCause::Crashed))
  });
  // Runs not continued are not this session's to recover: the next abnormal end does not list
  // them again.
  let mut desktop = crash_and_restart(desktop);
  desktop.wait_until("the project", |desktop| has(desktop, "nav-tasks"));
  for _ in 0..100 {
    desktop.tree.tick_futures();
    desktop.pass();
    std::thread::sleep(Duration::from_millis(20));
  }
  assert!(!has(&mut desktop, "recovery-dialog"));
  desktop.click("nav-tasks");
  desktop.wait_until("the task row", |desktop| {
    text_of(desktop, "task-row-KON-1", "Interrupted") && text_of(desktop, "task-row-KON-1", "No checkpoint yet")
  });
  desktop.click("task-row-KON-1");
  // Continue run asks the runner, which starts again with the application: until it is online,
  // the page answers that it is not connected.
  desktop.wait_until("Continue run with the runner online", |desktop| {
    has(desktop, "task-continue-run") && text_of(desktop, "status-runner", "Runner online")
  });
  desktop.click("task-continue-run");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
}

fn mark_agent_connection_unusable(repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("open project");
  let mut connection = db.connections().expect("connections").remove(0);
  connection
    .record_check(LastCheck::Agent(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::ExecutableMissing,
    }))
    .expect("agent check");
  db.save_connection(&connection).expect("save failed check");
}

/// Saves the project's limits with the defaults of Project settings and a daily budget of `daily`
/// tokens.
fn save_daily_budget(repo: &Path, daily: u64) {
  let limits = ProjectLimits::new(
    TokenBudget::new(10_000_000).expect("above zero"),
    TokenBudget::new(1_400_000).expect("above zero"),
    TokenBudget::new(daily).expect("above zero"),
    Duration::from_secs(30 * 60),
  )
  .expect("limits");
  ProjectDb::open(repo)
    .expect("open")
    .save_project_limits(&limits)
    .expect("save limits");
}

#[test]
fn lifecycle_the_daily_budget_stops_the_run_and_holds_back_new_work() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_claude(repo, &fake);
    save_daily_budget(repo, 5_000);
  });
  create_and_run(&mut desktop, "Work in turns.", "1400000");
  desktop.wait_until("the daily budget stop", |desktop| {
    latest_run(desktop.repo())
      .is_some_and(|run| run.status() == &RunStatus::Paused(PauseReason::LimitReached(RunLimit::DailyBudget)))
      && has(desktop, "task-resume-run")
  });
  desktop.wait_until("the notice", |desktop| {
    text_of(desktop, "task-notice", "Stopped at the daily project budget")
  });
  assert!(text_of(
    &mut desktop,
    "task-notice",
    "Builder was stopped after its turn and its work was committed as a checkpoint."
  ));
  assert!(text_of(
    &mut desktop,
    "task-progress-steps",
    "Stopped at limit · attempt 1 ·"
  ));
  assert!(!has(&mut desktop, "raise-limit"));
  assert!(has(&mut desktop, "task-cancel-run"));
  // Latest activity follows the brief and scrolls with the rows (`uv4PY`, `dKiEj`).
  assert!(text_of(&mut desktop, "task-progress-scroll", "Latest activity"));
  desktop.click("task-resume-run");
  let refusal = "Daily token budget used — new runs wait until tomorrow or raise it in Project settings.";
  desktop.wait_until("the refusal", |desktop| text_of(desktop, "task-page", refusal));
  // New work waits too: Create & run queues the task, and the runner refuses its run.
  desktop.click("task-breadcrumb");
  fill_new_task(&mut desktop, "Append one line to README.md.", "1400000");
  desktop.click("create-and-run");
  desktop.wait_until("the refusal in New task", |desktop| {
    desktop.tree.root().is_some_and(|root| contains_text(root, refusal)) && has(desktop, "create-and-run")
  });
  desktop.wait_until("the queued task status", |desktop| {
    text_of(desktop, "new-task-state", "Queued · run not started")
  });
  let tasks = ProjectDb::open(desktop.repo())
    .expect("open project")
    .tasks()
    .expect("tasks");
  assert_eq!(tasks.last().expect("new task").status(), TaskStatus::Queued);
  assert!(text_of(&mut desktop, "create-and-run", "Retry run"));
  assert!(!has(&mut desktop, "save-draft"));
  // Retry sends the same start request to the runner, which admits new work before it checks the
  // task's setup: with the daily budget raised, the retry reaches the preflight and reports what
  // it finds, and the task stays queued.
  mark_agent_connection_unusable(desktop.repo());
  save_daily_budget(desktop.repo(), 20_000_000);
  desktop.click("create-and-run");
  desktop.wait_until("the retry preflight error", |desktop| {
    text_of(desktop, "execution-plan", "connection that cannot be used now")
  });
  assert!(text_of(&mut desktop, "new-task-state", "Queued · run not started"));
  desktop.tab_to("create-and-run", 30);
  desktop.tab();
  assert_ne!(desktop.focused().as_deref(), Some("save-draft"));
  let tasks = ProjectDb::open(desktop.repo())
    .expect("open project")
    .tasks()
    .expect("tasks");
  assert_eq!(tasks.last().expect("new task").status(), TaskStatus::Queued);
}
