//! The project's browser with the real Chrome, Edge, Brave or Chromium of this computer, on pages the test serves on
//! loopback (`browser/site.rs`): it starts at the first use, opens a run's tab and reads it over CDP, keeps each run to
//! its own tabs, closes a run's tabs when the run ends (and not while it is paused), keeps nothing of a closed tab and
//! stops with the runner; the agents' browser tools through the relay (`browser/tools.rs`). Where no browser is found
//! each test says so and checks
//! nothing. Needs `git` for the scheduler's projects. Run with
//! `cargo test -p orchester-runner --features test-agent --test browser browser_`.

mod browser {
  pub mod site;
  mod tools;
}
mod support;

use std::{path::Path, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, RunId},
  run::{PauseReason, RunStatus},
};
use orchester_exec::browser::DevToolsActivePort;
use orchester_runner::browser::{
  BrowserError, Listening, ProjectBrowser,
  cdp::{Cdp, TargetId},
  profile_of,
};
use serde_json::json;
use tokio::net::TcpStream;

use self::{
  browser::site::serve as pages,
  support::{PATIENCE, Setup, pending::record_pending_run, project, scheduler, wait_for, wait_for_turns},
};

const POLL: Duration = Duration::from_millis(100);

/// Whether this computer has a browser; where it has none, the test says so and checks nothing.
#[allow(
  clippy::print_stderr,
  reason = "says that nothing was checked where no browser is installed"
)]
fn browser_installed() -> bool {
  match orchester_exec::browser::find() {
    Ok(_) => true,
    Err(unavailable) => {
      eprintln!("skipped, no browser to test with: {unavailable}");
      false
    }
  }
}

async fn title(browser: &ProjectBrowser, run: RunId, tab: &TargetId) -> String {
  let answer = browser
    .call(
      run,
      tab,
      "Runtime.evaluate",
      json!({ "expression": "document.title", "returnByValue": true }),
    )
    .await
    .expect("the page is evaluated");
  answer["result"]["value"].as_str().expect("a title").to_owned()
}

/// A connection of the test's own to the browser on `profile`, to see every tab it has.
async fn observer(profile: &Path) -> Cdp {
  let port = DevToolsActivePort::read(profile)
    .expect("readable")
    .expect("the browser listens");
  Cdp::connect(&port.websocket_url()).await.expect("connected")
}

async fn open_in_browser(observer: &Cdp, tab: &TargetId) -> bool {
  observer
    .targets()
    .await
    .expect("targets")
    .iter()
    .any(|target| &target.id == tab)
}

/// Waits until `tab` is gone from the browser.
async fn closed(observer: &Cdp, tab: &TargetId) {
  tokio::time::timeout(PATIENCE, async {
    while open_in_browser(observer, tab).await {
      tokio::time::sleep(POLL).await;
    }
  })
  .await
  .expect("the tab closes in time");
}

/// Waits until nothing listens on the browser's debugging port: the browser exited.
async fn exited(port: u16) {
  tokio::time::timeout(PATIENCE, async {
    while TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
      tokio::time::sleep(POLL).await;
    }
  })
  .await
  .expect("the browser exits in time");
}

/// Each project has its own profile folder, in the application data beside `orchester.db`, named by the project's id;
/// none is made until the browser starts.
#[test]
fn browser_profile_of_a_project_is_its_own_folder_in_the_application_data() {
  let project = project(&Setup::new("Say hello."));
  let profile = profile_of(project.repo.path()).expect("the folder is known");
  let data = orchester_store::location::app_data_location().expect("the application data");

  assert_eq!(
    profile,
    data
      .database
      .parent()
      .expect("the database's folder")
      .join("browser")
      .join(project.project.to_string())
  );
  assert!(!profile.starts_with(project.repo.path()), "never inside the repository");
  let unknown = tempfile::tempdir().expect("temporary folder");
  let refused = profile_of(unknown.path()).expect_err("not a project");
  assert!(refused.reason().starts_with("the project cannot be read"), "{refused}");
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_opens_a_runs_tab_reads_it_and_closes_the_runs_tabs_at_its_end() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let (ending, other) = (RunId::generate(), RunId::generate());
  assert!(
    browser.tabs(ending).await.expect("tabs").is_empty(),
    "nothing started yet"
  );

  let first = browser.open_tab(ending, &format!("{site}first")).await.expect("opened");
  let second = browser
    .open_tab(ending, &format!("{site}second"))
    .await
    .expect("opened");
  let others = browser.open_tab(other, &format!("{site}other")).await.expect("opened");

  assert_eq!(title(&browser, ending, &first).await, "Page first");
  assert_eq!(title(&browser, other, &others).await, "Page other");
  let listed = browser.tabs(ending).await.expect("tabs");
  let titles: Vec<&str> = listed.iter().map(|tab| tab.title.as_str()).collect();
  assert_eq!(titles, ["Page first", "Page second"]);
  assert_eq!(listed[1].url, format!("{site}second"));
  let refused = browser
    .call(other, &first, "Runtime.evaluate", json!({ "expression": "1" }))
    .await;
  assert!(
    matches!(refused, Err(BrowserError::NotRunsTab(tab)) if tab == first),
    "another run's tab"
  );

  let observer = observer(profile.path()).await;
  browser.close_run_tabs(ending).await.expect("closed");
  closed(&observer, &first).await;
  closed(&observer, &second).await;
  assert!(open_in_browser(&observer, &others).await, "the other run keeps its tab");
  assert!(browser.tabs(ending).await.expect("tabs").is_empty());
  assert!(matches!(
    browser
      .call(ending, &first, "Runtime.evaluate", json!({ "expression": "1" }))
      .await,
    Err(BrowserError::NotRunsTab(_))
  ));
  assert_eq!(title(&browser, other, &others).await, "Page other");

  let port = DevToolsActivePort::read(profile.path())
    .expect("readable")
    .expect("listening");
  browser.stop().await;
  exited(port.port()).await;
  assert!(
    browser.tabs(other).await.expect("tabs").is_empty(),
    "a stopped browser has no tabs"
  );
}

/// A program chosen in Settings › Browser that is no Chromium-based browser's is never passed over for the one
/// installed: the browser is unavailable and says why.
#[tokio::test(flavor = "multi_thread")]
async fn browser_chosen_in_settings_that_cannot_be_used_is_unavailable_with_the_reason() {
  let data = tempfile::tempdir().expect("temporary folder");
  let database = data.path().join("orchester.db");
  let gone = data.path().join("gone").join("chrome");
  orchester_store::AppDb::open(&database)
    .expect("the application database")
    .set_browser_program(Some(&gone))
    .expect("the choice is kept");
  let browser = ProjectBrowser::new(data.path().join("profile")).with_settings(database);

  let refused = browser
    .open_tab(RunId::generate(), "about:blank")
    .await
    .expect_err("the chosen program is gone");
  let BrowserError::Unavailable(unavailable) = refused else {
    panic!("unavailable, not {refused}");
  };
  assert_eq!(
    unavailable.reason(),
    format!(
      "the program chosen in Settings › Browser cannot be used: {} does not exist",
      gone.display()
    )
  );
  assert!(!data.path().join("profile").exists(), "nothing started");
}

/// The program chosen in Settings › Browser is the one that starts: here the installed one, chosen by its path.
#[tokio::test(flavor = "multi_thread")]
async fn browser_starts_the_program_chosen_in_settings() {
  if !browser_installed() {
    return;
  }
  let installed = orchester_exec::browser::find().expect("installed");
  let data = tempfile::tempdir().expect("temporary folder");
  let database = data.path().join("orchester.db");
  orchester_store::AppDb::open(&database)
    .expect("the application database")
    .set_browser_program(Some(installed.path()))
    .expect("the choice is kept");
  let browser = ProjectBrowser::new(data.path().join("profile")).with_settings(database);
  let site = pages().await;
  let run = RunId::generate();

  let tab = browser.open_tab(run, &format!("{site}chosen")).await.expect("opened");
  assert_eq!(title(&browser, run, &tab).await, "Page chosen");
  browser.stop().await;
}

/// Waits until `browser` listens to what `expected` says, its log followers ending on their own tasks.
async fn listening(browser: &ProjectBrowser, expected: Listening) {
  let settled = tokio::time::timeout(PATIENCE, async {
    while browser.listening().await != expected {
      tokio::time::sleep(POLL).await;
    }
  })
  .await;
  assert!(settled.is_ok(), "{:?}, not {expected:?}", browser.listening().await);
}

/// A tab that closes, by `browser_close`, at its run's end or by the browser itself, leaves nothing behind: its log's
/// follower ends and its events are no longer listened to, though the browser runs on.
#[tokio::test(flavor = "multi_thread")]
async fn browser_closed_tabs_leave_no_listener_or_log_behind() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let (ending, other) = (RunId::generate(), RunId::generate());
  let closing = browser
    .open_tab(other, &format!("{site}closing"))
    .await
    .expect("opened");
  for page in ["first", "second"] {
    browser
      .open_tab(ending, &format!("{site}{page}"))
      .await
      .expect("opened");
  }
  let kept = browser.open_tab(other, &format!("{site}kept")).await.expect("opened");
  // The runner's own receiver of the browser's events, and one per open tab.
  let open = |tabs| Listening {
    receivers: 1 + tabs,
    followed_logs: tabs,
  };
  listening(&browser, open(4)).await;

  browser.close_tab(other, &closing).await.expect("closed");
  listening(&browser, open(3)).await;
  browser.close_run_tabs(ending).await.expect("closed");
  listening(&browser, open(1)).await;
  let observer = observer(profile.path()).await;
  observer
    .close_target(&kept)
    .await
    .expect("closed by another connection");
  listening(&browser, open(0)).await;
  assert!(browser.tabs(other).await.expect("tabs").is_empty());

  browser.stop().await;
  assert_eq!(browser.listening().await, Listening::default());
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_ends_with_the_runner_that_drops_it_and_starts_again_at_the_next_use() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let site = pages().await;
  let run = RunId::generate();
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  browser.open_tab(run, &format!("{site}one")).await.expect("opened");
  let port = DevToolsActivePort::read(profile.path())
    .expect("readable")
    .expect("listening");

  drop(browser);
  exited(port.port()).await;

  let again = ProjectBrowser::new(profile.path().to_path_buf());
  let tab = again
    .open_tab(run, &format!("{site}two"))
    .await
    .expect("opened on the same profile");
  assert_eq!(title(&again, run, &tab).await, "Page two");
  again.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_tabs_of_a_cancelled_run_close_whether_or_not_its_task_runs() {
  if !browser_installed() {
    return;
  }
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let pending = record_pending_run(root, project.task).await;
  let profile = tempfile::tempdir().expect("temporary folder");
  let (scheduler, mut events) = scheduler(root);
  let scheduler = scheduler.with_browser_profile(profile.path().to_path_buf());
  let site = pages().await;
  let idle = scheduler
    .browser()
    .open_tab(pending.id(), &format!("{site}idle"))
    .await
    .expect("opened");
  let observer = observer(profile.path()).await;

  scheduler
    .cancel_run(pending.id())
    .await
    .expect("the pending run is cancelled");
  closed(&observer, &idle).await;

  let task = support::queue_another_task(root);
  let run = scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the run starts");
  wait_for_turns(root, &mut events, run, 1).await;
  let working = scheduler
    .browser()
    .open_tab(run, &format!("{site}working"))
    .await
    .expect("opened");
  scheduler.cancel_run(run).await.expect("the working run is cancelled");
  closed(&observer, &working).await;
  scheduler.browser().stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_tabs_of_a_paused_run_stay_and_close_once_it_completes() {
  if !browser_installed() {
    return;
  }
  let project = project(&Setup {
    review: true,
    ..Setup::new("Work slowly.")
  });
  let root = project.repo.path();
  let profile = tempfile::tempdir().expect("temporary folder");
  let (scheduler, mut events) = scheduler(root);
  let scheduler = scheduler.with_browser_profile(profile.path().to_path_buf());
  let site = pages().await;
  // Started before the run, so the run's tab opens within the agent's short turn.
  scheduler
    .browser()
    .open_tab(RunId::generate(), &format!("{site}warm"))
    .await
    .expect("the browser starts");
  let observer = observer(profile.path()).await;
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for_turns(root, &mut events, run, 1).await;
  let tab = scheduler
    .browser()
    .open_tab(run, &format!("{site}kept"))
    .await
    .expect("opened");

  scheduler.pause_run(run).await.expect("the pause is requested");
  let paused = wait_for(root, &mut events, run, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::Requested));
  tokio::time::sleep(Duration::from_millis(500)).await;
  assert!(open_in_browser(&observer, &tab).await, "pausing keeps the run's tabs");
  assert_eq!(title(scheduler.browser(), run, &tab).await, "Page kept");

  scheduler.resume_run(run).await.expect("the run resumes");
  let done = wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  closed(&observer, &tab).await;
  scheduler.browser().stop().await;
}
