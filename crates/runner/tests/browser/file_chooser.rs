//! A page's request for files with a real browser (`docs/design/browser.md`, "What frames do not show"): on the
//! person's tab the window is told what the field takes and its name, and its answer sets the chosen files on the
//! field or, cancelled, leaves it as it is; a request the tab no longer makes is refused. On an agent's tab no request
//! reaches the window: the chooser is cancelled and the tab's log, which the agent's next answer on it reads, says
//! that a file was asked.

use std::{path::PathBuf, time::Duration};

use orchester_core::id::RunId;
use orchester_ipc::{
  Rejection,
  browser::{BrowserCommand, BrowserEvent, BrowserTabId, FileChoice, FileChooser, Navigation},
};
use orchester_runner::browser::{FILE_ASKED, ProjectBrowser, cdp::TargetId};
use serde_json::json;

use super::{Desktop, PATIENCE, browser_installed, click, evaluate, pages, person_tab, tab_of};

const POLL: Duration = Duration::from_millis(100);
/// Where the page's fields are, in CSS pixels (`super::FILES`).
const ONE: (i32, i32) = (150, 20);
const MANY: (i32, i32) = (150, 120);

/// The person presses the field at `at` of `tab`, and the window is told what it asks for.
async fn ask(desktop: &mut Desktop, browser: &ProjectBrowser, tab: &BrowserTabId, at: (i32, i32)) -> FileChooser {
  click(desktop, browser, tab, at).await;
  let BrowserEvent::FileChooserOpened { tab: asking, chooser } = desktop
    .wait_for(|event| matches!(event, BrowserEvent::FileChooserOpened { .. }))
    .await
  else {
    unreachable!("waited for a request for files");
  };
  assert_eq!(&asking, tab);
  chooser
}

fn choose(tab: &BrowserTabId, chooser: &FileChooser, choice: FileChoice) -> BrowserCommand {
  BrowserCommand::ChooseFiles {
    tab: tab.clone(),
    chooser: chooser.id,
    choice,
  }
}

/// Files of the test's own, on this computer's paths.
fn files(folder: &tempfile::TempDir, names: &[&str]) -> Vec<PathBuf> {
  names
    .iter()
    .map(|name| {
      let path = folder.path().join(name);
      std::fs::write(&path, "-----BEGIN CERTIFICATE-----").expect("a file to choose");
      path
    })
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_file_chooser_of_the_persons_tab_is_answered_by_the_window() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let chosen = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let mut desktop = Desktop::new();
  desktop.serve(&browser, BrowserCommand::Subscribe).await;
  let tab = person_tab(&desktop, &browser, Some(format!("{site}window-files"))).await;
  desktop
    .state(|state| state.tabs.iter().any(|listed| listed.title == "Page files"))
    .await;

  let one = ask(&mut desktop, &browser, &tab, ONE).await;
  assert!(!one.multiple);
  assert_eq!(one.accept, [".pem", "application/x-pem-file"]);
  assert_eq!(one.label.as_deref(), Some("Signing certificate"));
  let pem = files(&chosen, &["kontur-hooks.pem"]);
  desktop
    .serve(&browser, choose(&tab, &one, FileChoice::Chosen { paths: pem.clone() }))
    .await;
  // The browser trims the title's trailing space.
  desktop
    .state(|state| state.tabs.iter().any(|listed| listed.title == "one kontur-hooks.pem many"))
    .await;
  let again = browser.arrived(choose(&tab, &one, FileChoice::Chosen { paths: pem }));
  let refused = browser.serve(again, &desktop.sender).await;
  assert!(matches!(refused, Err(Rejection::Failed { .. })), "answered already: {refused:?}");

  let cancelled = ask(&mut desktop, &browser, &tab, ONE).await;
  assert_ne!(cancelled.id, one.id, "each request is its own");
  desktop.serve(&browser, choose(&tab, &cancelled, FileChoice::Cancelled)).await;
  let many = ask(&mut desktop, &browser, &tab, MANY).await;
  assert!(many.multiple && many.accept.is_empty() && many.label.is_none(), "{many:?}");
  let both = files(&chosen, &["a.txt", "b.txt"]);
  desktop
    .serve(&browser, choose(&tab, &many, FileChoice::Chosen { paths: both }))
    .await;
  desktop
    .state(|state| {
      state
        .tabs
        .iter()
        .any(|listed| listed.title == "one kontur-hooks.pem many a.txt,b.txt")
    })
    .await;
  browser.stop().await;
}

/// A tab that navigates drops its request: the files chosen for it are refused.
#[tokio::test(flavor = "multi_thread")]
async fn browser_file_chooser_of_a_tab_that_navigates_is_dropped() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let chosen = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let mut desktop = Desktop::new();
  desktop.serve(&browser, BrowserCommand::Subscribe).await;
  let tab = person_tab(&desktop, &browser, Some(format!("{site}window-files"))).await;
  desktop
    .state(|state| state.tabs.iter().any(|listed| listed.title == "Page files"))
    .await;

  let asked = ask(&mut desktop, &browser, &tab, ONE).await;
  let away = BrowserCommand::Navigate {
    tab: tab.clone(),
    to: Navigation::Address {
      url: format!("{site}later"),
    },
  };
  desktop.serve(&browser, away).await;
  desktop
    .state(|state| state.tabs.iter().any(|listed| listed.title == "Later"))
    .await;
  let late = choose(
    &tab,
    &asked,
    FileChoice::Chosen {
      paths: files(&chosen, &["late.pem"]),
    },
  );
  let refused = browser.serve(browser.arrived(late), &desktop.sender).await;
  assert!(matches!(refused, Err(Rejection::Failed { .. })), "{refused:?}");
  browser.stop().await;
}

/// The files the agent's tab `target` of `run` asked for since the last look, as its next answer would say.
async fn asked_of_agent(browser: &ProjectBrowser, run: RunId, target: &TargetId) -> Vec<String> {
  tokio::time::timeout(PATIENCE, async {
    loop {
      let asked = browser.tab(run, Some(target)).await.expect("the run's tab").log().take_dialogs();
      if !asked.is_empty() {
        return asked;
      }
      tokio::time::sleep(POLL).await;
    }
  })
  .await
  .expect("the request is recorded in time")
}

/// An agent presses the field at `at` of its tab, as its browser tools do.
async fn agent_click(browser: &ProjectBrowser, run: RunId, target: &TargetId, (x, y): (i32, i32)) {
  for kind in ["mousePressed", "mouseReleased"] {
    browser
      .call(
        run,
        target,
        "Input.dispatchMouseEvent",
        json!({ "type": kind, "x": x, "y": y, "button": "left", "clickCount": 1 }),
      )
      .await
      .expect("the click reaches the page");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_file_chooser_of_an_agents_tab_is_cancelled_and_told_to_the_agent() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let mut desktop = Desktop::new();
  desktop.serve(&browser, BrowserCommand::Subscribe).await;
  let run = RunId::generate();
  let target = browser
    .open_tab(run, &format!("{site}window-files"))
    .await
    .expect("opened");

  agent_click(&browser, run, &target, ONE).await;
  assert_eq!(asked_of_agent(&browser, run, &target).await, [FILE_ASKED]);
  agent_click(&browser, run, &target, MANY).await;
  assert_eq!(
    asked_of_agent(&browser, run, &target).await,
    [FILE_ASKED],
    "a cancelled request leaves the next one free to come"
  );
  let chosen = evaluate(
    &browser,
    run,
    &target,
    "document.getElementById('one').files.length + document.getElementById('many').files.length",
  )
  .await;
  assert_eq!(chosen, 0, "no file was set");
  let told = desktop.received().iter().any(|event| {
    matches!(event, BrowserEvent::FileChooserOpened { tab, .. } if *tab == tab_of(&target))
  });
  assert!(!told, "the window is never asked for an agent's files");
  browser.stop().await;
}
