//! The project's browser with the real Chrome, Edge, Brave or Chromium of this computer, on pages the test serves on
//! loopback: it starts at the first use, opens a run's tab and reads it over CDP, keeps each run to its own tabs,
//! closes a run's tabs when the run ends (and not while it is paused) and stops with the runner; the *Browser* window's
//! side of it is in [`window`]. Where no browser is found each test says so and checks nothing. Needs `git` for the
//! scheduler's projects. Run with `cargo test -p orchester-runner --features test-agent --test browser browser_`.

mod support;
#[path = "browser/window.rs"]
mod window;

use std::{path::Path, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, RunId},
  run::{PauseReason, RunStatus},
};
use orchester_exec::browser::DevToolsActivePort;
use orchester_runner::browser::{
  BrowserError, ProjectBrowser,
  cdp::{Cdp, TargetId},
  profile_of,
};
use serde_json::json;
use tokio::{
  io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
  net::{TcpListener, TcpStream},
};

use self::support::{PATIENCE, Setup, pending::record_pending_run, project, scheduler, wait_for, wait_for_turns};

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

/// Serves `<title>Page <name></title>` at `/<name>` on loopback until the test ends, and the window tests' pages at
/// their names; the address to put names after.
async fn pages() -> String {
  let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback port");
  let address = listener.local_addr().expect("its address");
  tokio::spawn(async move {
    while let Ok((stream, _)) = listener.accept().await {
      tokio::spawn(answer(stream));
    }
  });
  format!("http://{address}/")
}

async fn answer(stream: TcpStream) {
  let mut reader = BufReader::new(stream);
  let mut request = String::new();
  if reader.read_line(&mut request).await.is_err() {
    return;
  }
  let mut header = String::new();
  while reader.read_line(&mut header).await.is_ok_and(|read| read > 2) {
    header.clear();
  }
  let name = request.split_whitespace().nth(1).unwrap_or("/").trim_start_matches('/');
  let body = match name {
    "form" => window::FORM.to_owned(),
    "animated" => window::ANIMATED.to_owned(),
    "dialog" => window::DIALOG.to_owned(),
    _ => format!("<!doctype html><html><head><title>Page {name}</title></head><body>{name}</body></html>"),
  };
  let response = format!(
    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
    body.len()
  );
  // A browser that stopped reading leaves nothing to answer.
  let _ = reader.get_mut().write_all(response.as_bytes()).await;
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
