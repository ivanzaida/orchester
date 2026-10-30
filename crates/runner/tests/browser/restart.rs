//! The person's navigation, and the restart in a browser window of its own and back, which reopens the tabs by their
//! addresses (`docs/design/browser.md`, decision 7). The restart is only ever run back to headless here: headed, it
//! would open a real window on the screen of whoever runs the tests.

use orchester_core::id::RunId;
use orchester_ipc::{
  Rejection,
  browser::{BrowserCommand, BrowserEvent, BrowserState, BrowserTabId, Navigation, TabOwner},
};
use orchester_runner::browser::ProjectBrowser;

use super::{Desktop, PATIENCE, browser_installed, pages, person_tab, viewport};

/// The person's address line and history, and the restart that reopens the tabs by their addresses for their owners.
#[tokio::test(flavor = "multi_thread")]
async fn browser_person_navigates_and_the_restart_reopens_the_tabs() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let mut desktop = Desktop::new();
  desktop.serve(&browser, BrowserCommand::Subscribe).await;
  let run = RunId::generate();
  browser.open_tab(run, &format!("{site}kept")).await.expect("opened");
  let tab = person_tab(&desktop, &browser, Some(format!("{site}first"))).await;
  let at = |title: &'static str| move |state: &BrowserState| state.tabs.iter().any(|tab| tab.title == title);
  desktop.state(at("Page first")).await;

  let to = Navigation::Address {
    url: format!("{site}second"),
  };
  desktop
    .serve(&browser, BrowserCommand::Navigate { tab: tab.clone(), to })
    .await;
  desktop.state(at("Page second")).await;
  let back = BrowserCommand::Navigate {
    tab: tab.clone(),
    to: Navigation::Back,
  };
  desktop.serve(&browser, back).await;
  desktop.state(at("Page first")).await;
  let forward = BrowserCommand::Navigate {
    tab: tab.clone(),
    to: Navigation::Forward,
  };
  desktop.serve(&browser, forward.clone()).await;
  desktop.state(at("Page second")).await;
  let none = browser.serve(browser.arrived(forward), &desktop.sender).await;
  assert!(
    matches!(none, Err(Rejection::Failed { .. })),
    "no page forward: {none:?}"
  );

  desktop.serve(&browser, BrowserCommand::BackToOrchester).await;
  let state = desktop
    .state(|state| {
      state.tabs.len() == 2
        && state
          .tabs
          .iter()
          .all(|listed| listed.title.starts_with("Page") && listed.id != tab)
    })
    .await;
  let reopened: Vec<(TabOwner, &str)> = state.tabs.iter().map(|tab| (tab.owner, tab.title.as_str())).collect();
  assert_eq!(
    reopened,
    [(TabOwner::Run { run }, "Page kept"), (TabOwner::Person, "Page second")]
  );
  assert_eq!(browser.tabs(run).await.expect("tabs").len(), 1, "the run keeps its tab");
  browser.stop().await;
}

/// The window keeps showing a tab through the restart (`05` `GjcAk`): it hears nothing until the tabs are back, never
/// of a browser without them, and the tab shown is still the one shown, under its new id, screencast once it asks.
#[tokio::test(flavor = "multi_thread")]
async fn browser_restart_keeps_the_tab_shown_and_tells_the_window_when_the_tabs_are_back() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let mut desktop = Desktop::new();
  desktop.serve(&browser, BrowserCommand::Subscribe).await;
  person_tab(&desktop, &browser, Some(format!("{site}first"))).await;
  let tab = person_tab(&desktop, &browser, Some(format!("{site}second"))).await;
  let show = |tab: BrowserTabId| BrowserCommand::Show {
    tab,
    viewport: viewport(),
  };
  desktop.serve(&browser, show(tab.clone())).await;
  desktop.state(|state| state.shown.as_ref() == Some(&tab)).await;
  let before = desktop.received().len();

  desktop.serve(&browser, BrowserCommand::BackToOrchester).await;
  let state = desktop
    .state(|state| {
      state.tabs.len() == 2
        && state
          .tabs
          .iter()
          .all(|listed| listed.title.starts_with("Page") && listed.id != tab)
    })
    .await;
  let shown = state.shown.expect("a tab stays shown");
  let second = state.tabs.iter().find(|listed| listed.title == "Page second");
  assert_eq!(Some(&shown), second.map(|listed| &listed.id), "the same page");
  for event in &desktop.received()[before..] {
    if let BrowserEvent::Changed(state) = event {
      assert_eq!(state.tabs.len(), 2, "never a browser without its tabs: {state:?}");
      assert!(state.shown.is_some(), "never no tab shown: {state:?}");
    }
  }

  desktop.serve(&browser, show(shown.clone())).await;
  let frame = tokio::time::timeout(PATIENCE, browser.next_frame())
    .await
    .expect("a frame in time");
  assert_eq!(frame.tab, shown);
  browser.stop().await;
}
