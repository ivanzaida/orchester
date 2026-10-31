//! When the window stops the screencast (`docs/design/browser.md`, "The page"): while it is minimised, and for good
//! when it closes. The window's requests are kept instead of sent to a runner.

use orchester_ipc::browser::{BrowserCommand, BrowserMode, BrowserTabId};
use serde_json::json;

use super::{
  inspection::Tools,
  support::{Browser, browser_installed, state},
};

/// The window on the person's tab, past the `Show` it asks by itself once the page area is measured.
fn showing() -> Option<Browser> {
  if !browser_installed() {
    return None;
  }
  let mut browser = Browser::open(false);
  browser.state.record_commands_for_test();
  browser.feed(state("P1", BrowserMode::Headless).0);
  browser.until_shown("browser-page");
  browser.until("the window to ask for the tab", |browser| {
    matches!(asked(browser).as_slice(), [BrowserCommand::Show { .. }])
  });
  Some(browser)
}

fn asked(browser: &Browser) -> Vec<BrowserCommand> {
  browser
    .state
    .recorded_commands_for_test()
    .into_iter()
    .map(|(root, command)| {
      assert_eq!(root, browser.root, "the request goes to the project shown");
      command
    })
    .collect()
}

#[test]
fn browser_minimising_stops_the_screencast_and_restoring_shows_the_tab_again() {
  let Some(mut browser) = showing() else {
    return;
  };
  browser.link.minimize_for_test(true);
  browser.pass();
  assert_eq!(asked(&browser), [BrowserCommand::StopShowing]);
  browser.pass();
  assert_eq!(asked(&browser), [], "it asks once");

  browser.link.minimize_for_test(false);
  browser.pass();
  let again = asked(&browser);
  assert!(
    matches!(again.as_slice(), [BrowserCommand::Show { tab, .. }] if *tab == BrowserTabId::new("P1")),
    "the same tab again, not {again:?}"
  );
}

#[test]
fn browser_closing_stops_the_screencast() {
  let Some(mut browser) = showing() else {
    return;
  };
  let tools = Tools::new(&mut browser);
  let answer = tools.call(&mut browser, "lurq_interact", &json!({"action": "request_close"}));
  assert!(
    answer.to_string().contains("close_queued"),
    "the window closes: {answer}"
  );
  assert_eq!(asked(&browser), [BrowserCommand::StopShowing]);
  assert!(!browser.link.is_open(), "the next request opens it again");
}
