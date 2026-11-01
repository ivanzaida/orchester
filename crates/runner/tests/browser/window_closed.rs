//! The browser in a window of its own that the person closes, or that ends, instead of *Back to Orchester*
//! (`docs/design/browser.md`, decision 7): it starts again headless on the same profile with its tabs, their owners and
//! the tab shown. Here its window is out of sight ([`ProjectBrowser::with_windows_out_of_sight`]), so nothing opens on
//! the screen of whoever runs the tests; the ignored test at the end closes a real one.

use std::time::Duration;

use orchester_core::id::RunId;
use orchester_exec::browser::DevToolsActivePort;
use orchester_ipc::{
  Rejection,
  browser::{BrowserCommand, BrowserMode, BrowserState, BrowserTabId, TabOwner},
};
use orchester_runner::browser::{
  BrowserError, ProjectBrowser,
  cdp::{Cdp, CdpError, TargetId},
};
use serde_json::json;

use super::{Desktop, PATIENCE, browser_installed, evaluate, observer, pages, person_tab, viewport};
use crate::browser::lifecycle::{browser_process, kill};

/// A browser `desktop` follows, in a window of its own as *Open in a browser window* leaves it, with a tab of `run` on
/// `/kept` and a tab of the person's, shown, on `/shown`: that tab of the run's, by its id before and after the window
/// opened.
async fn in_its_window(
  browser: &ProjectBrowser,
  desktop: &mut Desktop,
  site: &str,
  run: RunId,
) -> (TargetId, TargetId) {
  desktop.serve(browser, BrowserCommand::Subscribe).await;
  let first = browser.open_tab(run, &format!("{site}kept")).await.expect("opened");
  person_tab(desktop, browser, Some(format!("{site}shown"))).await;
  desktop.state(|state| titled(state, "Page shown")).await;
  desktop.serve(browser, BrowserCommand::OpenInWindow).await;
  let state = desktop
    .state(|state| state.mode == BrowserMode::Headed && titled(state, "Page kept") && titled(state, "Page shown"))
    .await;
  let shown = tab_titled(&state, "Page shown");
  let show = BrowserCommand::Show {
    tab: shown.clone(),
    viewport: viewport(),
  };
  desktop.serve(browser, show).await;
  desktop.state(|state| state.shown.as_ref() == Some(&shown)).await;
  let opened = TargetId::from(tab_titled(&state, "Page kept").as_str());
  (first, opened)
}

fn titled(state: &BrowserState, title: &str) -> bool {
  state.tabs.iter().any(|tab| tab.title == title)
}

fn tab_titled(state: &BrowserState, title: &str) -> BrowserTabId {
  let found = state.tabs.iter().find(|tab| tab.title == title);
  found
    .unwrap_or_else(|| panic!("no tab titled {title} in {state:?}"))
    .id
    .clone()
}

/// Waits until the browser came back headless with its two tabs loaded, and checks their owners and the tab shown.
async fn came_back(desktop: &mut Desktop, run: RunId, before: &[BrowserTabId]) -> BrowserState {
  let state = desktop
    .state(|state| {
      state.mode == BrowserMode::Headless
        && titled(state, "Page kept")
        && titled(state, "Page shown")
        && state.tabs.iter().all(|tab| !before.contains(&tab.id))
    })
    .await;
  let reopened: Vec<(TabOwner, &str)> = state.tabs.iter().map(|tab| (tab.owner, tab.title.as_str())).collect();
  assert_eq!(
    reopened,
    [(TabOwner::Run { run }, "Page kept"), (TabOwner::Person, "Page shown")]
  );
  assert_eq!(
    state.shown,
    Some(tab_titled(&state, "Page shown")),
    "the tab shown stays"
  );
  state
}

/// The ids the browser in its own window gave its tabs.
fn ids_now(desktop: &mut Desktop) -> Vec<BrowserTabId> {
  let seen = desktop.received().iter().rev().find_map(|event| match event {
    orchester_ipc::browser::BrowserEvent::Changed(state) if state.mode == BrowserMode::Headed => Some(state.clone()),
    _ => None,
  });
  seen
    .expect("a state of the browser in its window")
    .tabs
    .into_iter()
    .map(|tab| tab.id)
    .collect()
}

/// Closes every page of the browser from another connection, as the person's closing its window does: its tabs are
/// reported closed one by one, and the browser may run on, as it does on macOS.
async fn close_every_page(observer: &Cdp) {
  for target in observer.targets().await.expect("targets") {
    if target.kind == "page" {
      observer
        .close_target(&target.id)
        .await
        .expect("closed by another connection");
    }
  }
}

/// A browser killed from outside: nothing reports its tabs closed, and it comes back with them; a run's next call works
/// by the id its tab had, by the id it has now and without naming one, and its current tab is the page it was.
#[tokio::test(flavor = "multi_thread")]
async fn browser_in_its_window_that_is_killed_comes_back_headless_with_its_tabs() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf()).with_windows_out_of_sight();
  let site = pages().await;
  let mut desktop = Desktop::new();
  let run = RunId::generate();
  let (first, in_window) = in_its_window(&browser, &mut desktop, &site, run).await;
  let before = ids_now(&mut desktop);
  let killed = browser_process(profile.path()).await;

  kill(killed);

  let state = came_back(&mut desktop, run, &before).await;
  assert_ne!(browser_process(profile.path()).await, killed, "a new process");
  let page = |tab| evaluate(&browser, run, tab, "document.title");
  assert_eq!(page(&first).await, "Page kept", "by the id it had first");
  assert_eq!(page(&in_window).await, "Page kept", "by the id it had in the window");
  let now = TargetId::from(tab_titled(&state, "Page kept").as_str());
  assert_eq!(page(&now).await, "Page kept", "by the id it has now");
  let current = browser.tab(run, None).await.expect("the run's current tab");
  assert_eq!(current.id(), &now);
  assert_eq!(browser.tabs(run).await.expect("tabs").len(), 1, "the run keeps its tab");
  browser.stop().await;
}

/// The person closes the window: every page is reported closed first and the browser may run on without a window, yet
/// no tab counts as closed, and each comes back on the address it had.
#[tokio::test(flavor = "multi_thread")]
async fn browser_whose_window_the_person_closes_comes_back_headless_with_its_tabs() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf()).with_windows_out_of_sight();
  let site = pages().await;
  let mut desktop = Desktop::new();
  let run = RunId::generate();
  let (first, _) = in_its_window(&browser, &mut desktop, &site, run).await;
  let before = ids_now(&mut desktop);

  close_every_page(&observer(profile.path()).await).await;

  came_back(&mut desktop, run, &before).await;
  assert_eq!(evaluate(&browser, run, &first, "document.title").await, "Page kept");
  browser.stop().await;
}

/// One tab the person closes in the window, while the browser has other pages, is closed for good, and stays so for the
/// run through a later return of the browser.
#[tokio::test(flavor = "multi_thread")]
async fn browser_keeps_a_tab_closed_alone_in_its_window_closed_when_it_comes_back() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf()).with_windows_out_of_sight();
  let site = pages().await;
  let mut desktop = Desktop::new();
  let run = RunId::generate();
  let (_, in_window) = in_its_window(&browser, &mut desktop, &site, run).await;
  let observer = observer(profile.path()).await;

  observer
    .close_target(&in_window)
    .await
    .expect("closed by another connection");
  desktop.state(|state| state.tabs.len() == 1).await;
  let refused = browser
    .tab(run, Some(&in_window))
    .await
    .err()
    .expect("the tab is closed");
  assert!(matches!(&refused, BrowserError::ClosedInBrowser(_)), "{refused:?}");

  close_every_page(&observer).await;
  let state = desktop
    .state(|state| state.mode == BrowserMode::Headless && titled(state, "Page shown"))
    .await;
  assert_eq!(state.tabs.len(), 1, "only the person's tab comes back: {state:?}");
  let refused = browser.tab(run, Some(&in_window)).await.err().expect("still closed");
  assert!(matches!(&refused, BrowserError::ClosedInBrowser(_)), "{refused:?}");
  browser.stop().await;
}

/// A cookie set while the browser is in its window is in the profile the browser comes back on. The browser closes on
/// its own, as it does when its last window closes, so it keeps what it saved; a killed browser may lose the last ones.
#[tokio::test(flavor = "multi_thread")]
async fn browser_keeps_a_cookie_set_in_its_window_when_it_comes_back() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf()).with_windows_out_of_sight();
  let site = pages().await;
  let mut desktop = Desktop::new();
  let run = RunId::generate();
  let (first, _) = in_its_window(&browser, &mut desktop, &site, run).await;
  let before = ids_now(&mut desktop);
  evaluate(&browser, run, &first, "document.cookie = 'kept=yes; max-age=3600'").await;

  let closing = observer(profile.path()).await;
  let closed = closing.call(None, "Browser.close", json!({})).await;
  // The browser may close the connection before it answers.
  assert!(matches!(closed, Ok(_) | Err(CdpError::Closed)), "{closed:?}");

  came_back(&mut desktop, run, &before).await;
  let cookie = evaluate(&browser, run, &first, "document.cookie").await;
  assert_eq!(cookie, "kept=yes", "the cookie is still there");
  browser.stop().await;
}

/// A browser that cannot start again: the window learns that no tabs are left, and every next use says why.
#[tokio::test(flavor = "multi_thread")]
async fn browser_that_cannot_come_back_from_its_window_is_unavailable_with_the_reason() {
  if !browser_installed() {
    return;
  }
  let installed = orchester_exec::browser::find().expect("installed");
  let data = tempfile::tempdir().expect("temporary folder");
  let database = data.path().join("orchester.db");
  let mut settings = orchester_store::AppDb::open(&database).expect("the application database");
  settings
    .set_browser_program(Some(installed.path()))
    .expect("the choice is kept");
  let profile = data.path().join("profile");
  let browser = ProjectBrowser::new(profile.clone())
    .with_settings(database)
    .with_windows_out_of_sight();
  let site = pages().await;
  let mut desktop = Desktop::new();
  let run = RunId::generate();
  let (first, _) = in_its_window(&browser, &mut desktop, &site, run).await;
  let gone = data.path().join("gone").join("chrome");
  settings.set_browser_program(Some(&gone)).expect("the choice is kept");

  kill(browser_process(&profile).await);

  desktop.state(|state| *state == BrowserState::default()).await;
  let refused = browser.tab(run, Some(&first)).await.err().expect("no browser");
  let BrowserError::Unavailable(unavailable) = refused else {
    panic!("unavailable, not {refused}");
  };
  assert!(unavailable.reason().contains("does not exist"), "{unavailable}");
  let opened = browser.open_tab(run, &format!("{site}after")).await;
  assert!(matches!(opened, Err(BrowserError::Unavailable(_))), "{opened:?}");
  let open = BrowserCommand::OpenTab { url: None };
  let refused = browser.serve(browser.arrived(open), &desktop.sender).await;
  assert!(
    matches!(refused, Err(Rejection::BrowserUnavailable { .. })),
    "{refused:?}"
  );
  browser.stop().await;
}

/// Orchester stopping the browser in its window does not bring it back, and the window learns that it has no tabs.
#[tokio::test(flavor = "multi_thread")]
async fn browser_in_its_window_that_orchester_stops_does_not_come_back() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf()).with_windows_out_of_sight();
  let site = pages().await;
  let mut desktop = Desktop::new();
  let run = RunId::generate();
  in_its_window(&browser, &mut desktop, &site, run).await;
  let port = DevToolsActivePort::read(profile.path())
    .expect("readable")
    .expect("listening");

  browser.stop().await;

  desktop.state(|state| *state == BrowserState::default()).await;
  crate::exited(port.port()).await;
  tokio::time::sleep(Duration::from_secs(1)).await;
  assert!(
    DevToolsActivePort::read(profile.path())
      .expect("readable")
      .is_none_or(|now| now == port),
    "no browser started again"
  );
  assert!(browser.tabs(run).await.expect("tabs").is_empty(), "no browser runs");
}

/// With a real window, which opens on the screen: closing its pages closes it, whatever the platform then does with the
/// process, and the browser comes back. Run by hand with `-- --ignored`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "opens a real browser window on the screen; run separately"]
async fn browser_with_a_real_window_that_closes_comes_back_headless_with_its_tabs() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let mut desktop = Desktop::new();
  let run = RunId::generate();
  let (first, _) = in_its_window(&browser, &mut desktop, &site, run).await;
  let before = ids_now(&mut desktop);
  evaluate(&browser, run, &first, "document.cookie = 'kept=yes; max-age=3600'").await;

  close_every_page(&observer(profile.path()).await).await;

  tokio::time::timeout(PATIENCE, came_back(&mut desktop, run, &before))
    .await
    .expect("the browser comes back in time");
  let cookie = evaluate(&browser, run, &first, "document.cookie").await;
  assert_eq!(cookie, "kept=yes", "the cookie is still there");
  browser.stop().await;
}
