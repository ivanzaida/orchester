//! Closing the window and starting again (layer "App lifecycle" `Ob1P7`), headless at 1440 × 900. With no
//! agent at work a close stops the runners at once and the session ends clean. With agents of
//! `orchester-fake-codex` at work (the QA seed's `lifecycle` scenario, the real runner) the close is held
//! and the closing panel takes the window's body: Esc cancels and the page comes back; Wait for agents to
//! save stops them at their checkpoints, the window closes once they are saved, the runs are paused and
//! the next start offers nothing; Quit now ends the agents and their processes at once, and the next start
//! offers the interrupted run on the project's Overview, where Continue all takes it up again.
//!
//! The live tests need the runner and its fakes: `cargo build -p orchester-runner --features test-agent
//! --bins`. The window's own close request (its ×, Alt+F4, the app's MCP `request_close`) reaches the same
//! flow through `lurq`'s close handler, which a headless tree cannot raise: these tests ask the flow
//! directly.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers the other scenarios use")]
mod kontur;
mod support;

use std::{
  path::{Path, PathBuf},
  process::Command,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
  time::{Duration, Instant},
};

use lurq::app::events::MouseButton;
use orchester_app::{services::Services, startup::Launch};
use orchester_core::{
  connection::stream::AgentEvent,
  id::{ProjectId, RunId},
  run::{InterruptCause, RunStatus},
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_store::{PreviousSession, ProjectDb, SessionEnd};
use orchester_workspace::{
  app_mcp::{self, Translate},
  close::ClosePhase,
  recovery::{Decision, Reading},
  runner::state::RunnerState,
  session::AppSession,
};
use serde_json::{Value, json};
use support::{Data, Window};
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// How long a real runner and agent may take for one step of a test.
const PATIENCE: Duration = Duration::from_secs(60);

/// The runner and the fake Codex beside the test's own programs.
fn programs() -> (PathBuf, PathBuf) {
  let folder = std::env::current_exe()
    .expect("the test's path")
    .parent()
    .and_then(Path::parent)
    .expect("the target folder")
    .to_path_buf();
  let program = |name: &str| folder.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
  let (runner, codex) = (program("orchester-runner"), program("orchester-fake-codex"));
  assert!(
    runner.is_file() && codex.is_file(),
    "build the runner and its fakes first: cargo build -p orchester-runner --features test-agent --bins"
  );
  (runner, codex)
}

/// A window on the seeded `lifecycle` scenario with the real runner, and what the test closes it with.
struct Live {
  window: Window,
  runner: PathBuf,
  project: ProjectId,
  repo: PathBuf,
}

impl Live {
  fn seeded() -> Self {
    let (runner, codex) = programs();
    let dir = TempDir::new().expect("a temporary folder");
    let request = kontur::SeedRequest {
      data_dir: dir.path().to_path_buf(),
      repo: dir.path().join("repo-lifecycle"),
      scenario: kontur::scenarios::named("lifecycle").expect("a known scenario"),
      fake_codex: Some(codex),
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let data = Data {
      dir,
      runtime: Runtime::new().expect("tokio runtime"),
    };
    let mut live = Self::open(data, &runner, seeded.project);
    live.repo = seeded.repo;
    live
  }

  /// The window of a new session on `data`.
  fn open(data: Data, runner: &Path, project: ProjectId) -> Self {
    let launch = Launch::begin_at(&data.database(), runner.to_path_buf(), data.runtime.handle(), None);
    let repo = data.dir.path().join("repo-lifecycle");
    Self {
      window: Window::of(data, launch),
      runner: runner.to_path_buf(),
      project,
      repo,
    }
  }

  fn services(&self) -> Services {
    self.window.launch.services().expect("the session started").clone()
  }

  /// As the window's `until`, for work a real runner and agent do.
  fn wait(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = Instant::now() + PATIENCE;
    loop {
      self.window.tree.tick_futures();
      self.window.pass();
      if done(self) {
        return;
      }
      assert!(Instant::now() < deadline, "timed out waiting for {what}");
      thread::sleep(Duration::from_millis(20));
    }
  }

  /// Starts a run of task `key` as the app's `run_task` does, once the project's runner is online.
  fn run(&mut self, key: &str) -> RunId {
    let services = self.services();
    let repo = self.repo.clone();
    self.wait("the runner online", |_| {
      services
        .runners
        .status_of(&repo)
        .is_some_and(|status| *status.state() == RunnerState::Online)
    });
    let domain = app_mcp::Services::new(
      services.workspace.clone(),
      services.runners.clone(),
      services.servers.clone(),
      services.programs.clone(),
      (
        services.changes.clone(),
        services.activity.clone(),
        services.jobs.clone(),
      ),
    );
    let translate = Translate::new(|message| message.key.to_owned());
    let arguments = json!({"project": self.project.to_string(), "key": key});
    let Value::Object(arguments) = arguments else {
      unreachable!("a JSON object")
    };
    let answer = self
      .window
      .data
      .runtime
      .block_on(app_mcp::call(&domain, &translate, "run_task", arguments))
      .expect("run_task is a domain tool")
      .unwrap_or_else(|error| panic!("{key} runs: {error:?}"));
    answer["run"]["id"]
      .as_str()
      .and_then(|id| id.parse().ok())
      .expect("the run's id")
  }

  /// Waits until the runner reports `run`'s agent at work.
  fn at_work(&mut self, run: RunId) {
    let services = self.services();
    self.wait("the agent at work", |_| {
      services.runners.active_work().iter().any(|work| work.run() == run)
    });
  }

  /// Presses the window's own close control (the title bar's ×, or the macOS close light).
  fn press_close(&mut self) {
    self.click("window-close");
  }

  /// Whether the close flow let the window close, and how the session ended then.
  fn closed(&self) -> bool {
    self.services().close.session_end().is_some()
  }

  fn key(&mut self, key: &str) {
    self
      .window
      .tree
      .key_down(key.to_owned(), key.to_owned(), false, false, false);
    self
      .window
      .tree
      .key_up(key.to_owned(), key.to_owned(), false, false, false);
    self.window.pass();
  }

  fn click(&mut self, id: &str) {
    let bounds = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    self.window.tree.mouse_move(x, y);
    self.window.tree.mouse_down(x, y, MouseButton::Left);
    self.window.tree.mouse_up(x, y, MouseButton::Left);
    self.window.pass();
  }

  fn text(&mut self, id: &str) -> String {
    self.window.texts(id).unwrap_or_default().join(" ")
  }

  fn has(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  fn run_status(&self, run: RunId) -> RunStatus {
    ProjectDb::open(&self.repo)
      .expect("the project's database")
      .run(run)
      .expect("read the run")
      .expect("the run")
      .status()
      .clone()
  }

  /// Waits for the window to be let close, ends the session as `main` does, and starts the app again on
  /// the same data.
  fn reopen(mut self) -> Self {
    self.wait("the window let close", |live| live.closed());
    let Live {
      window,
      runner,
      project,
      repo,
      ..
    } = self;
    let Window { launch, data, .. } = window;
    launch.end();
    let mut live = Self::open(data, &runner, project);
    live.repo = repo;
    live
  }
}

/// How the last session ended, as a new one reads it; it ends that one at once.
fn previous_end(database: &Path) -> PreviousSession {
  let session = AppSession::start_at(database).expect("a session starts");
  let previous = session.previous();
  session.end(SessionEnd::Clean).expect("the session ends");
  previous
}

/// Whether process `pid` still runs.
fn is_running(pid: &str) -> bool {
  let output = if cfg!(windows) {
    Command::new("tasklist")
      .args(["/FI", &format!("PID eq {pid}"), "/NH"])
      .output()
  } else {
    Command::new("kill").args(["-0", pid]).output()
  }
  .expect("ask the system about the process");
  if cfg!(windows) {
    String::from_utf8_lossy(&output.stdout).contains(pid)
  } else {
    output.status.success()
  }
}

/// The lines `run`'s agent wrote to standard error that start with `prefix`, the prefix left out.
fn reported(repo: &Path, run: RunId, prefix: &str) -> Vec<String> {
  ProjectDb::open(repo)
    .expect("the project's database")
    .activity(run, 0)
    .expect("the run's activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::Raw { line, .. } => line.strip_prefix(prefix).map(str::to_owned),
      _ => None,
    })
    .collect()
}

#[test]
fn lifecycle_close_without_work_stops_quietly_and_ends_clean() {
  let mut window = Window::open();
  let services = window.launch.services().expect("the session started").clone();
  let closed = Arc::new(AtomicBool::new(false));
  let letting = closed.clone();
  services.close.request(move || letting.store(true, Ordering::Release));
  window.until("the window let close", |_| closed.load(Ordering::Acquire));
  assert!(window.tree.get_element_by_id("closing-panel").is_none());
  assert_eq!(services.close.session_end(), Some(SessionEnd::Clean));
  let Window { launch, data, .. } = window;
  launch.end();
  assert_eq!(
    previous_end(&data.database()),
    PreviousSession::Ended(SessionEnd::Clean)
  );
}

#[test]
fn lifecycle_close_with_agents_working_waits_for_them_to_save_and_the_runs_stay_paused() {
  let mut live = Live::seeded();
  let run = live.run("KON-2");
  live.at_work(run);

  live.press_close();
  live.wait("the closing panel", |live| live.has(&format!("closing-row-{run}")));
  assert_eq!(live.text("closing-title"), "Close Orchester while agents are working?");
  assert!(
    live
      .text(&format!("closing-row-{run}"))
      .contains("Save at a checkpoint")
  );
  assert!(live.text(&format!("closing-status-{run}")).contains("Running"));
  assert!(live.has("closing"), "the closing panel lies over the page");

  live.key("Escape");
  live.wait("the page back", |live| !live.has("closing-panel"));
  assert_eq!(live.services().close.phase(), ClosePhase::Open);
  assert!(!live.closed(), "Cancel keeps the window open");
  assert!(live.has("sidebar"));

  live.press_close();
  live.wait("the closing panel again", |live| live.has("closing-wait"));
  live.key("Enter");
  live.wait("the agents saving", |live| {
    live.text("closing-title") == "Saving before closing"
      && live
        .text(&format!("closing-status-{run}"))
        .contains("Saving at next checkpoint")
  });
  assert!(live.text("closing-progress").contains("Closes after 1 more run"));
  live.press_close();
  assert!(
    !live.closed(),
    "the window's × changes nothing more while the panel is shown"
  );

  std::fs::write(live.repo.join(".orchester").join("fake-release"), "released").expect("release the fake");
  let database = live.window.data.database();
  let live = live.reopen();
  assert!(
    matches!(live.run_status(run), RunStatus::Paused(_)),
    "{:?}",
    live.run_status(run)
  );
  assert_eq!(live.services().recovery.state().reading, Reading::Nothing);
  let Live { window, .. } = live;
  let Window { launch, .. } = window;
  launch.end();
  assert_eq!(previous_end(&database), PreviousSession::Ended(SessionEnd::Clean));
}

#[test]
fn lifecycle_quit_now_while_agents_save_ends_them_and_the_next_starts_offer_their_runs_until_continued() {
  let mut live = Live::seeded();
  let first = live.run("KON-1");
  live.at_work(first);
  let repo = live.repo.clone();
  let mut helper = None;
  live.wait("the agent's helper", |_| {
    helper = reported(&repo, first, "helper pid: ").pop();
    helper.is_some()
  });
  let helper = helper.expect("the helper's pid");
  let agent = reported(&repo, first, "pid: ").pop().expect("the agent's pid");
  assert!(is_running(&agent) && is_running(&helper));

  live.press_close();
  live.wait("the closing panel", |live| live.has(&format!("closing-row-{first}")));
  // A run that starts while the panel is shown is listed too.
  let second = live.run("KON-2");
  live.wait("the run that started", |live| {
    live.has(&format!("closing-row-{second}"))
  });
  live.key("Enter");
  live.wait("the agents saving", |live| {
    live.text("closing-title") == "Saving before closing"
  });
  assert!(live.text("closing-progress").contains("Closes after 2 more runs"));
  // Tab moves to Quit now, the one choice left while the agents save; Enter presses it.
  live.key("Tab");
  live.key("Enter");
  assert_eq!(live.services().close.session_end(), Some(SessionEnd::QuitNow));
  live.wait("the window let close", |live| live.closed());
  let deadline = Instant::now() + Duration::from_secs(10);
  while is_running(&agent) && Instant::now() < deadline {
    thread::sleep(Duration::from_millis(100));
  }
  assert!(!is_running(&agent), "Quit now ends the agent");
  // Ending a process tree is a known gap on macOS (`AGENTS.md`); the helper outlives the agent there.
  if cfg!(windows) {
    assert!(!is_running(&helper), "and what it started");
  }
  // As a macOS runner does when its app quits: it records its runs as interrupted before the next start.
  interrupt(&repo, &[first, second]);

  let mut live = live.reopen();
  live.wait("the interrupted runs offered", |live| live.has("recovery-review"));
  assert_eq!(
    live.text("recovery-notice-title"),
    "2 runs were interrupted when Orchester closed"
  );
  // Nothing decided: the notice comes back after a clean restart.
  live.press_close();
  let mut live = live.reopen();
  live.wait("the runs offered again", |live| live.has("recovery-review"));
  live.click("recovery-review");
  live.wait("the list in place", |live| live.has(&format!("recovery-run-{first}")));
  assert!(
    live
      .text(&format!("recovery-run-{first}"))
      .contains("You chose Quit now")
  );

  live.click("recovery-continue");
  let services = live.services();
  let project = live.project;
  live.wait("the runs continued", |_| {
    matches!(services.recovery.state().decision(project), Decision::Continued(_))
  });
  let Decision::Continued(continuation) = services.recovery.state().decision(project) else {
    unreachable!("waited for it")
  };
  assert_eq!((continuation.continued, continuation.not_continued.len()), (2, 0));
  live.wait("the notice gone", |live| !live.has("recovery-notice"));
  for run in [first, second] {
    assert!(
      !matches!(live.run_status(run), RunStatus::Interrupted(_)),
      "the run is live again"
    );
  }

  live.press_close();
  live.wait("the window closing or asking", |live| {
    live.closed() || live.has("closing-quit-now")
  });
  if live.has("closing-quit-now") {
    live.click("closing-quit-now");
  }
  live.wait("the window let close at the end", |live| live.closed());
  let Live { window, .. } = live;
  let Window { launch, .. } = window;
  launch.end();
}

/// Records `runs` of the project in `repo` as interrupted by Quit now, now.
fn interrupt(repo: &Path, runs: &[RunId]) {
  let mut db = ProjectDb::open(repo).expect("the project's database");
  for id in runs {
    let mut run = db.run(*id).expect("read the run").expect("the run");
    if matches!(run.status(), RunStatus::Interrupted(_)) {
      continue;
    }
    let events = run
      .interrupt(InterruptCause::QuitNow, Timestamp::now())
      .expect("a run at work can be interrupted");
    db.append_events(&run, &events).expect("record the interruption");
  }
}
