//! A run's tab that someone other than the run closed, with a real browser (`docs/design/browser.md`, "Tabs"): the
//! run's next call on it says who closed it, the person from the window or the browser, and points to `browser_open`;
//! one the run closed itself keeps the answer of any tab that is not the run's, and nothing of the closed tabs is kept
//! once the run ends.

use orchester_core::id::RunId;
use orchester_ipc::browser::{BrowserCommand, BrowserTabId};
use orchester_runner::browser::{BrowserError, Listening, ProjectBrowser, cdp::TargetId};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::site;
use crate::{browser_installed, listening, observer, title};

/// What the browser listens to with no tab open: the runner's own receiver of its events. A closed tab is no longer
/// kept once it is so.
const NOTHING_OPEN: Listening = Listening {
  receivers: 1,
  followed_logs: 0,
};

async fn evaluated(
  browser: &ProjectBrowser,
  run: RunId,
  tab: &TargetId,
  expression: &str,
) -> Result<Value, BrowserError> {
  browser
    .call(
      run,
      tab,
      "Runtime.evaluate",
      json!({ "expression": expression, "returnByValue": true }),
    )
    .await
}

/// Closes `tab` as the *Browser* window's command does.
async fn close_from_window(browser: &ProjectBrowser, tab: &TargetId) {
  let (desktop, _events) = mpsc::unbounded_channel();
  let close = BrowserCommand::CloseTab {
    tab: BrowserTabId::new(tab.as_str()),
  };
  browser
    .serve(browser.arrived(close), &desktop)
    .await
    .expect("the window closes the tab");
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_runs_tab_closed_from_the_window_answers_that_the_person_closed_it() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = site::serve().await;
  let (run, other) = (RunId::generate(), RunId::generate());
  let closed = browser.open_tab(run, &format!("{site}closed")).await.expect("opened");
  let kept = browser.open_tab(run, &format!("{site}kept")).await.expect("opened");

  close_from_window(&browser, &closed).await;

  let refused = evaluated(&browser, run, &closed, "1")
    .await
    .expect_err("the tab is closed");
  assert!(
    matches!(&refused, BrowserError::ClosedByPerson(tab) if tab == &closed),
    "{refused:?}"
  );
  let said = refused.to_string();
  assert!(
    said.contains("the person closed tab") && said.contains("browser_open"),
    "{said}"
  );
  let again = browser.close_tab(run, &closed).await.expect_err("already closed");
  assert!(matches!(again, BrowserError::ClosedByPerson(_)), "{again:?}");
  let others = evaluated(&browser, other, &closed, "1")
    .await
    .expect_err("never the other run's");
  assert!(matches!(others, BrowserError::NotRunsTab(_)), "{others:?}");
  let listed: Vec<TargetId> = browser
    .tabs(run)
    .await
    .expect("tabs")
    .into_iter()
    .map(|tab| tab.id)
    .collect();
  assert_eq!(listed, std::slice::from_ref(&kept), "the run keeps its other tab");
  assert_eq!(title(&browser, run, &kept).await, "Page kept");
  browser.stop().await;
}

/// The browser reports the tab closed, as when its page closes itself or the person closes it in the browser's own
/// window. A page of a run's tab cannot close itself in Chrome (`window.close()` is refused in a tab whose history
/// starts with its blank page), so another connection to the browser closes it here.
#[tokio::test(flavor = "multi_thread")]
async fn browser_runs_tab_closed_in_the_browser_answers_that_it_was_closed_there() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = site::serve().await;
  let run = RunId::generate();
  let tab = browser.open_tab(run, &format!("{site}closing")).await.expect("opened");

  observer(profile.path())
    .await
    .close_target(&tab)
    .await
    .expect("closed by another connection");
  listening(&browser, NOTHING_OPEN).await;

  let refused = evaluated(&browser, run, &tab, "1")
    .await
    .expect_err("the tab is closed");
  assert!(
    matches!(&refused, BrowserError::ClosedInBrowser(closed) if closed == &tab),
    "{refused:?}"
  );
  let said = refused.to_string();
  assert!(
    said.contains("by its own page") && said.contains("browser_open"),
    "{said}"
  );
  browser.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_runs_tab_it_closed_itself_keeps_the_answer_of_a_tab_not_its() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = site::serve().await;
  let run = RunId::generate();
  let tab = browser.open_tab(run, &format!("{site}own")).await.expect("opened");

  browser.close_tab(run, &tab).await.expect("closed");
  // Once the browser reported it closed too, it is still the run's own closing.
  listening(&browser, NOTHING_OPEN).await;

  let refused = evaluated(&browser, run, &tab, "1").await.expect_err("closed");
  assert!(
    matches!(&refused, BrowserError::NotRunsTab(closed) if closed == &tab),
    "{refused:?}"
  );
  browser.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_runs_tab_closed_by_the_person_is_forgotten_when_the_run_ends() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = site::serve().await;
  let run = RunId::generate();
  let closed = browser.open_tab(run, &format!("{site}closed")).await.expect("opened");
  browser.open_tab(run, &format!("{site}open")).await.expect("opened");
  close_from_window(&browser, &closed).await;
  let refused = evaluated(&browser, run, &closed, "1").await.expect_err("closed");
  assert!(matches!(refused, BrowserError::ClosedByPerson(_)), "{refused:?}");

  browser.close_run_tabs(run).await.expect("the run's tabs close");

  let refused = evaluated(&browser, run, &closed, "1").await.expect_err("closed");
  assert!(
    matches!(&refused, BrowserError::NotRunsTab(tab) if tab == &closed),
    "nothing is kept of the ended run: {refused:?}"
  );
  browser.stop().await;
}
