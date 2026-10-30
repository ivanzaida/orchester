//! The browser's process as the runner keeps it: one killed from outside is started again at the next use, on the same
//! profile, without the killed one's tabs; and each project's browser is a process of its own, which stops on its own.

use std::{path::Path, process::Command};

use orchester_core::id::RunId;
use orchester_exec::browser::DevToolsActivePort;
use orchester_runner::browser::{BrowserError, Listening, ProjectBrowser};
use serde_json::json;

use super::site::serve as pages;
use crate::{browser_installed, exited, listening, observer, open_in_browser, title};

/// The process id of the browser running on `profile`, as the browser itself reports it.
pub async fn browser_process(profile: &Path) -> u64 {
  let info = observer(profile)
    .await
    .call(None, "SystemInfo.getProcessInfo", json!({}))
    .await
    .expect("the browser's processes");
  info["processInfo"]
    .as_array()
    .into_iter()
    .flatten()
    .find(|process| process["type"] == "browser")
    .and_then(|process| process["id"].as_u64())
    .unwrap_or_else(|| panic!("no browser process in {info}"))
}

/// Kills the process `pid` at once, as a person or another program outside the runner might.
pub fn kill(pid: u64) {
  let pid = pid.to_string();
  #[cfg(unix)]
  let killed = Command::new("kill").args(["-KILL", &pid]).status();
  #[cfg(windows)]
  let killed = Command::new("taskkill").args(["/F", "/PID", &pid]).status();
  assert!(killed.expect("the kill runs").success(), "{pid} was killed");
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_killed_from_outside_starts_again_at_the_next_use() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let run = RunId::generate();
  let before = browser.open_tab(run, &format!("{site}before")).await.expect("opened");
  let port = DevToolsActivePort::read(profile.path())
    .expect("readable")
    .expect("listening");
  let killed = browser_process(profile.path()).await;

  kill(killed);
  exited(port.port()).await;
  // The runner saw its connection end; nothing starts the browser again until it is used.
  listening(&browser, Listening::default()).await;

  let after = browser
    .open_tab(run, &format!("{site}after"))
    .await
    .expect("opened in the browser started again");
  assert_eq!(title(&browser, run, &after).await, "Page after");
  assert_ne!(browser_process(profile.path()).await, killed, "a new process");
  let gone = browser
    .call(run, &before, "Runtime.evaluate", json!({ "expression": "1" }))
    .await;
  assert!(
    matches!(gone, Err(BrowserError::NotRunsTab(tab)) if tab == before),
    "the killed browser's tab is gone"
  );
  browser.stop().await;
}

/// Two projects' browsers, on their own profile folders, are two processes listening on two ports: each has only its
/// own tabs, and stopping one leaves the other running.
#[tokio::test(flavor = "multi_thread")]
async fn browser_of_each_project_is_a_process_of_its_own() {
  if !browser_installed() {
    return;
  }
  let profiles = [
    tempfile::tempdir().expect("temporary folder"),
    tempfile::tempdir().expect("temporary folder"),
  ];
  let browsers = profiles
    .each_ref()
    .map(|profile| ProjectBrowser::new(profile.path().to_path_buf()));
  let site = pages().await;
  let (first_run, second_run) = (RunId::generate(), RunId::generate());
  let first = browsers[0]
    .open_tab(first_run, &format!("{site}first-project"))
    .await
    .expect("opened");
  let second = browsers[1]
    .open_tab(second_run, &format!("{site}second-project"))
    .await
    .expect("opened");

  let ports = profiles.each_ref().map(|profile| {
    DevToolsActivePort::read(profile.path())
      .expect("readable")
      .expect("listening")
      .port()
  });
  assert_ne!(ports[0], ports[1], "each browser listens on a port of its own");
  assert_ne!(
    browser_process(profiles[0].path()).await,
    browser_process(profiles[1].path()).await,
    "each project's browser is a process of its own"
  );
  let (first_observer, second_observer) = (observer(profiles[0].path()).await, observer(profiles[1].path()).await);
  assert!(open_in_browser(&first_observer, &first).await);
  assert!(
    !open_in_browser(&first_observer, &second).await,
    "not the other project's tab"
  );
  assert!(open_in_browser(&second_observer, &second).await);
  assert!(
    !open_in_browser(&second_observer, &first).await,
    "not the other project's tab"
  );

  browsers[0].stop().await;
  exited(ports[0]).await;
  assert_eq!(
    title(&browsers[1], second_run, &second).await,
    "Page second-project",
    "the other project's browser runs on"
  );
  browsers[1].stop().await;
}
