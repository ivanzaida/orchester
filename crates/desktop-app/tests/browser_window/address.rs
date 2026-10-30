//! What the address line shows and whether it holds the focus as tabs come and go: *Empty* gives it the focus (`06`),
//! which goes when a tab comes, so the line shows that tab's address as `01` and `02` draw it; text the person typed
//! and Ctrl+L on a page keep it.

use orchester_ipc::browser::{BrowserEvent, BrowserMode, BrowserState, BrowserTabId, TabOwner};

use super::support::{Browser, browser_installed, tab};

const FOCUSED: Option<&str> = Some("browser-address");

/// The window on *Empty*, with the focus in its address line.
fn on_empty() -> Option<Browser> {
  if !browser_installed() {
    return None;
  }
  let mut browser = Browser::open(false);
  browser.feed(BrowserEvent::Changed(BrowserState::default()));
  browser.until_shown("browser-empty");
  browser.pass();
  assert_eq!(browser.focused().as_deref(), FOCUSED);
  Some(browser)
}

/// The runner reports the window's only tab `T1` at the address `path` of `localhost:5173`, titled `path` so the strip
/// tells when the window has it; the blur a replaced input sends is delivered too.
fn go_to(browser: &mut Browser, path: &str) {
  let mut only = tab("T1", TabOwner::Person, path);
  only.url = format!("http://localhost:5173/{path}");
  browser.feed(BrowserEvent::Changed(BrowserState {
    tabs: vec![only],
    shown: Some(BrowserTabId::new("T1")),
    mode: BrowserMode::Headless,
  }));
  browser.until("the window to have the new address", |browser| {
    browser.shows("browser-tab-0") && browser.text("browser-tab-0").contains(path)
  });
  browser.pass();
  browser.pass();
}

fn value(browser: &mut Browser) -> String {
  browser
    .tree
    .get_element_by_id_mut("browser-address")
    .and_then(|element| element.as_text_input())
    .expect("the address line is a text input")
    .value()
}

fn type_in(browser: &mut Browser, typed: &str) {
  browser
    .tree
    .get_element_by_id_mut("browser-address")
    .and_then(|element| element.as_text_input())
    .expect("the address line is a text input")
    .set_value(typed);
  browser.pass();
}

fn enter(browser: &mut Browser) {
  browser
    .tree
    .key_down("Enter".to_owned(), "Enter".to_owned(), false, false, false);
  browser.pass();
}

#[test]
fn browser_address_line_lets_go_of_the_focus_and_follows_when_a_tab_comes_after_empty() {
  let Some(mut browser) = on_empty() else {
    return;
  };
  assert_eq!(value(&mut browser), "");
  go_to(&mut browser, "orders");
  assert_eq!(browser.focused(), None, "the page shows with no focus ring (01, 02)");
  assert_eq!(value(&mut browser), "localhost:5173/orders");

  go_to(&mut browser, "endpoints");
  assert_eq!(
    value(&mut browser),
    "localhost:5173/endpoints",
    "and follows from then on"
  );
  assert_eq!(browser.focused(), None);

  browser.feed(BrowserEvent::Changed(BrowserState::default()));
  browser.until_shown("browser-empty");
  browser.pass();
  assert_eq!(
    browser.focused().as_deref(),
    FOCUSED,
    "Empty again waits for an address"
  );
  assert_eq!(value(&mut browser), "");
}

#[test]
fn browser_address_line_keeps_what_the_person_typed_when_a_tab_comes_after_empty() {
  let Some(mut browser) = on_empty() else {
    return;
  };
  type_in(&mut browser, "docs.rs/req");
  go_to(&mut browser, "orders");
  assert_eq!(browser.focused().as_deref(), FOCUSED);
  assert_eq!(value(&mut browser), "docs.rs/req");
  go_to(&mut browser, "endpoints");
  assert_eq!(browser.focused().as_deref(), FOCUSED);
  assert_eq!(
    value(&mut browser),
    "docs.rs/req",
    "typing is not overwritten by the tab"
  );
}

#[test]
fn browser_address_line_keeps_the_focus_ctrl_l_gave_it_on_a_page() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  go_to(&mut browser, "orders");
  assert_eq!(browser.focused(), None);
  browser.command("l");
  browser.pass();
  assert_eq!(browser.focused().as_deref(), FOCUSED);

  go_to(&mut browser, "endpoints");
  assert_eq!(browser.focused().as_deref(), FOCUSED);
  assert_eq!(value(&mut browser), "localhost:5173/endpoints", "nothing typed yet");

  type_in(&mut browser, "docs.rs");
  go_to(&mut browser, "orders");
  assert_eq!(browser.focused().as_deref(), FOCUSED);
  assert_eq!(value(&mut browser), "docs.rs");
}

#[test]
fn browser_address_line_follows_the_tab_after_enter_opened_it_from_empty() {
  let Some(mut browser) = on_empty() else {
    return;
  };
  browser.state.record_commands_for_test();
  type_in(&mut browser, "localhost:5173/orders");
  enter(&mut browser);
  assert_eq!(browser.focused(), None, "Enter leaves the line");
  assert_eq!(value(&mut browser), "localhost:5173/orders", "until the tab comes");
  go_to(&mut browser, "orders-opened");
  assert_eq!(value(&mut browser), "localhost:5173/orders-opened");
  assert_eq!(browser.focused(), None);
}

#[test]
fn browser_address_line_keeps_the_focus_when_enter_has_nothing_to_open() {
  let Some(mut browser) = on_empty() else {
    return;
  };
  browser.state.record_commands_for_test();
  enter(&mut browser);
  assert_eq!(browser.focused().as_deref(), FOCUSED);
  assert_eq!(browser.state.recorded_commands_for_test(), [], "nothing was asked");
}
