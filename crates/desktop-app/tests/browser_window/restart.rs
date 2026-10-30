//! The seconds a restart takes, the runner's answer held back (`docs/design/browser.md`, "Opened in a browser window"):
//! the pressed button turns its loader and cannot be pressed again, and the toolbar and the shortcuts ask nothing,
//! until the answer gives them back; a refused restart says so on the failure line.

use lurq::app::events::MouseButton;
use orchester_app::ui::icon::Icon;
use orchester_ipc::{
  Rejection,
  browser::{BrowserCommand, BrowserMode, BrowserReply},
};

use super::support::{Browser, browser_installed, state};

const OPEN: &str = "browser-open-in-window";
const BACK: &str = "browser-back-to-orchester";

/// The window on `mode`'s browser, its requests kept with their answers held, and past the `Show` it asks by itself
/// once the page area is measured.
fn holding(mode: BrowserMode) -> Browser {
  let mut browser = Browser::open(false);
  browser.state.record_commands_for_test();
  browser.state.hold_answers_for_test();
  browser.feed(state("P1", mode).0);
  browser.until("the page area to be measured", |browser| {
    !browser.state.recorded_commands_for_test().is_empty()
  });
  browser
}

fn asked(browser: &Browser) -> Vec<BrowserCommand> {
  browser
    .state
    .recorded_commands_for_test()
    .into_iter()
    .map(|(_, command)| command)
    .collect()
}

fn press(browser: &mut Browser, id: &str) {
  let (x, y) = browser.bounds(id).center();
  browser.tree.mouse_down(x, y, MouseButton::Left);
  browser.tree.mouse_up(x, y, MouseButton::Left);
  browser.pass();
}

/// The button's loader turns: it is `state=loading`, which the tree shows only to tools.
fn loading(browser: &mut Browser, id: &str) -> bool {
  browser
    .tree
    .get_element_by_id(&format!("{id}-icon"))
    .and_then(|icon| icon.text_content().map(str::to_owned))
    == Some(Icon::LoaderCircle.glyph().to_string())
}

/// Presses `button` once more and every shortcut.
fn press_everything(browser: &mut Browser, button: &str) {
  press(browser, button);
  for key in ["t", "w", "r", "l"] {
    browser.command(key);
  }
  browser
    .tree
    .key_down("Tab".to_owned(), "Tab".to_owned(), false, true, false);
  browser
    .tree
    .key_up("Tab".to_owned(), "Tab".to_owned(), false, true, false);
  browser.pass();
}

/// `button` was pressed and its answer is held: it shows that, and nothing more is asked of the browser.
fn assert_pending(browser: &mut Browser, button: &str) {
  browser.until("the button's loader", |browser| loading(browser, button));
  press_everything(browser, button);
  assert_eq!(asked(browser), [], "nothing more is asked meanwhile");
  assert_ne!(
    browser.focused().as_deref(),
    Some("browser-address"),
    "the toolbar takes no input meanwhile"
  );
  assert!(loading(browser, button), "{button} still turns its loader");
}

fn until_toolbar_takes_input(browser: &mut Browser) {
  browser.until("the toolbar to take input again", |browser| {
    browser.command("l");
    browser.focused().as_deref() == Some("browser-address")
  });
}

fn refusal() -> Result<BrowserReply, Rejection> {
  Err(Rejection::BrowserUnavailable {
    detail: "the program is gone".to_owned(),
  })
}

/// *Open in a browser window* asks once and holds the toolbar until the answer: back with an Ok, back with a refusal,
/// which the failure line says.
#[test]
fn browser_open_in_a_browser_window_holds_the_toolbar_until_the_answer_ok_or_refused() {
  if !browser_installed() {
    return;
  }
  let mut browser = holding(BrowserMode::Headless);

  press(&mut browser, OPEN);
  assert_eq!(asked(&browser), [BrowserCommand::OpenInWindow]);
  assert_pending(&mut browser, OPEN);
  browser.state.answer_held_for_test(&Ok(BrowserReply::Done));
  until_toolbar_takes_input(&mut browser);
  assert!(!loading(&mut browser, OPEN), "the loader stops with the answer");
  assert!(!browser.shows("browser-failed"));
  assert_eq!(asked(&browser), [], "the toolbar's way back asks nothing");

  press(&mut browser, OPEN);
  assert_eq!(asked(&browser), [BrowserCommand::OpenInWindow]);
  browser.until("the address line to let go", |browser| {
    browser.focused().as_deref() != Some("browser-address")
  });
  assert_pending(&mut browser, OPEN);
  browser.state.answer_held_for_test(&refusal());
  browser.until_shown("browser-failed");
  until_toolbar_takes_input(&mut browser);
  assert!(!loading(&mut browser, OPEN), "the loader stops with the refusal");
}

/// *Back to Orchester* in `05` asks once and turns its loader until the answer: a refusal gives the button back and
/// says so on the failure line; with an Ok the browser is headless, as the runner reported just before it, and the
/// toolbar takes input again.
#[test]
fn browser_back_to_orchester_holds_until_the_answer_ok_or_refused() {
  if !browser_installed() {
    return;
  }
  let mut browser = holding(BrowserMode::Headed);
  browser.until_shown(BACK);

  press(&mut browser, BACK);
  assert_eq!(asked(&browser), [BrowserCommand::BackToOrchester]);
  assert_pending(&mut browser, BACK);
  browser.state.answer_held_for_test(&refusal());
  browser.until_shown("browser-failed");
  browser.until("the loader to stop with the refusal", |browser| !loading(browser, BACK));
  // The failure line takes room from the page area, whose new size the window names with the tab shown.
  browser.until("the page area to be measured again", |browser| {
    !asked(browser).is_empty()
  });

  press(&mut browser, BACK);
  assert_eq!(asked(&browser), [BrowserCommand::BackToOrchester], "pressed again");
  assert_pending(&mut browser, BACK);
  browser.feed(state("P1", BrowserMode::Headless).0);
  browser.until_shown(OPEN);
  assert!(loading(&mut browser, OPEN), "the state came, the answer did not yet");
  browser.command("l");
  assert_ne!(browser.focused().as_deref(), Some("browser-address"));
  browser.state.answer_held_for_test(&Ok(BrowserReply::Done));
  until_toolbar_takes_input(&mut browser);
  assert!(!loading(&mut browser, OPEN), "the loader stops with the answer");
  assert!(!browser.shows("browser-failed"), "the request was carried out");
}
