//! What a run's tab records of its console: at most [`KEPT`] messages, the oldest dropped first.

use std::time::Duration;

use orchester_core::id::RunId;
use orchester_runner::browser::{KEPT, ProjectBrowser};
use serde_json::json;

use super::site::serve as pages;
use crate::{browser_installed, support::PATIENCE};

const POLL: Duration = Duration::from_millis(100);

#[tokio::test(flavor = "multi_thread")]
async fn browser_tab_log_keeps_its_newest_console_messages_up_to_its_limit() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let run = RunId::generate();
  let tab = browser.open_tab(run, &format!("{site}quiet")).await.expect("opened");
  let logged = KEPT + 5;
  browser
    .call(
      run,
      &tab,
      "Runtime.evaluate",
      json!({ "expression": format!("for (let i = 0; i < {logged}; i++) console.log('line ' + i)") }),
    )
    .await
    .expect("evaluated");

  let last = format!("line {}", logged - 1);
  let texts = tokio::time::timeout(PATIENCE, async {
    loop {
      let texts: Vec<String> = {
        let tab = browser.tab(run, Some(&tab)).await.expect("the run's tab");
        let log = tab.log();
        log.console().map(|message| message.text.clone()).collect()
      };
      // The log records on a task of its own: the last message recorded, every earlier one was.
      if texts.last() == Some(&last) {
        return texts;
      }
      tokio::time::sleep(POLL).await;
    }
  })
  .await
  .expect("every message is recorded in time");

  assert_eq!(texts.len(), KEPT);
  assert_eq!(texts[0], "line 5", "the five oldest were dropped");
  browser.stop().await;
}
