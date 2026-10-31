//! The all-tabs menu and the window's focus (`01d` `fWhIr`): its filter takes the focus when it opens, so what is typed
//! filters its rows and nothing reaches the tab, and the focus goes back where it was however the menu closes.

use lurq::app::events::MouseButton;
use orchester_ipc::browser::{
  BrowserCommand, BrowserDialog, BrowserEvent, BrowserMode, BrowserState, BrowserTab, BrowserTabId, DialogKind,
  TabOwner,
};

use super::support::{Browser, browser_installed, tab};

const FILTER: Option<&str> = Some("browser-overflow-filter");
const PAGE: Option<&str> = Some("browser-page-text");
const ADDRESS: Option<&str> = Some("browser-address");

/// A person's tabs named `Pull requests · GitHub`, and two about Rust at the end: `P0` is shown.
fn crowded() -> Vec<BrowserTab> {
  let mut tabs: Vec<BrowserTab> = (0..18)
    .map(|index| tab(&format!("P{index}"), TabOwner::Person, "Pull requests · GitHub"))
    .collect();
  tabs.push(tab("R1", TabOwner::Person, "Client in reqwest - Rust"));
  tabs.push(tab("R2", TabOwner::Person, "tokio::time::sleep - Rust"));
  tabs
}

fn report(tabs: Vec<BrowserTab>) -> BrowserEvent {
  BrowserEvent::Changed(BrowserState {
    tabs,
    shown: Some(BrowserTabId::new("P0")),
    mode: BrowserMode::Headless,
  })
}

/// The window on a crowded strip, its requests kept instead of sent.
fn on_crowded_strip() -> Option<Browser> {
  if !browser_installed() {
    return None;
  }
  let mut browser = Browser::open(false);
  browser.state.record_commands_for_test();
  browser.feed(report(crowded()));
  browser.until_shown("browser-overflow");
  browser.until_shown("browser-page");
  Some(browser)
}

/// The window with the focus on the page, by a press and a key there, which reached the tab.
fn on_used_page() -> Option<Browser> {
  let mut browser = on_crowded_strip()?;
  browser.use_page();
  assert_eq!(browser.focused().as_deref(), PAGE);
  assert!(
    inputs(&browser).iter().any(|input| input.contains("KeyA")),
    "the page had the keys"
  );
  Some(browser)
}

/// The person's inputs the window asked the tab to take since the last look.
fn inputs(browser: &Browser) -> Vec<String> {
  browser
    .state
    .recorded_commands_for_test()
    .into_iter()
    .filter_map(|(_, command)| match command {
      BrowserCommand::Input { input, .. } => Some(format!("{input:?}")),
      _ => None,
    })
    .collect()
}

fn press(browser: &mut Browser, id: &str) {
  let (x, y) = browser.bounds(id).center();
  browser.tree.mouse_down(x, y, MouseButton::Left);
  browser.tree.mouse_up(x, y, MouseButton::Left);
}

fn open_menu(browser: &mut Browser) {
  press(browser, "browser-overflow");
  browser.until_shown("browser-overflow-menu");
  browser.pass();
}

fn tap(browser: &mut Browser, key: &str, code: &str) {
  browser
    .tree
    .key_down(key.to_owned(), code.to_owned(), false, false, false);
  browser
    .tree
    .key_up(key.to_owned(), code.to_owned(), false, false, false);
  browser.pass();
}

/// Types `text`, whose keys need no shift for what this test reads of them.
fn type_keys(browser: &mut Browser, text: &str) {
  for letter in text.chars() {
    let code = format!("Key{}", letter.to_ascii_uppercase());
    tap(browser, &letter.to_string(), &code);
  }
}

fn filter_value(browser: &mut Browser) -> String {
  browser
    .tree
    .get_element_by_id_mut("browser-overflow-filter")
    .and_then(|element| element.as_text_input())
    .expect("the filter is a text input")
    .value()
}

/// The labels of the menu's rows.
fn rows(browser: &mut Browser) -> Vec<String> {
  (0..crowded().len())
    .filter_map(|index| {
      let id = format!("browser-overflow-{index}");
      browser.shows(&id).then(|| browser.text(&id))
    })
    .collect()
}

#[test]
fn browser_overflow_filter_takes_the_focus_and_the_keys_from_the_page() {
  let Some(mut browser) = on_used_page() else {
    return;
  };
  open_menu(&mut browser);
  assert_eq!(browser.focused().as_deref(), FILTER, "the filter has the focus (01d)");
  assert_eq!(rows(&mut browser).len(), crowded().len());

  type_keys(&mut browser, "rust");
  assert_eq!(filter_value(&mut browser), "rust");
  browser.until("the rows to be filtered", |browser| rows(browser).len() == 2);
  let shown = rows(&mut browser);
  assert!(
    shown[0].contains("Client in reqwest - Rust") && shown[1].contains("tokio::time::sleep - Rust"),
    "only the tabs that match: {shown:?}"
  );
  assert_eq!(inputs(&browser), Vec::<String>::new(), "no key reaches the tab");
}

#[test]
fn browser_overflow_escape_gives_the_page_its_focus_back_and_sends_it_nothing() {
  let Some(mut browser) = on_used_page() else {
    return;
  };
  open_menu(&mut browser);
  type_keys(&mut browser, "r");
  tap(&mut browser, "Escape", "Escape");
  browser.until("the menu to close", |browser| !browser.shows("browser-overflow-menu"));
  browser.pass();
  assert_eq!(browser.focused().as_deref(), PAGE);
  assert_eq!(
    inputs(&browser),
    Vec::<String>::new(),
    "neither the press nor the release of Esc is the page's"
  );

  type_keys(&mut browser, "b");
  let typed = inputs(&browser);
  assert_eq!(
    typed.len(),
    2,
    "the next key is the page's, pressed and released: {typed:?}"
  );
}

#[test]
fn browser_overflow_choosing_a_row_gives_the_focus_back() {
  let Some(mut browser) = on_used_page() else {
    return;
  };
  open_menu(&mut browser);
  press(&mut browser, "browser-overflow-3");
  browser.until("the menu to close", |browser| !browser.shows("browser-overflow-menu"));
  browser.pass();
  assert_eq!(browser.focused().as_deref(), PAGE);
}

/// The press that closes a menu only closes it, so the focus goes back where it was, though that was not the page.
#[test]
fn browser_overflow_press_outside_gives_the_address_line_its_focus_back() {
  let Some(mut browser) = on_crowded_strip() else {
    return;
  };
  browser.command("l");
  assert_eq!(browser.focused().as_deref(), ADDRESS);
  open_menu(&mut browser);
  assert_eq!(browser.focused().as_deref(), FILTER);
  press(&mut browser, "browser-page");
  browser.until("the menu to close", |browser| !browser.shows("browser-overflow-menu"));
  browser.pass();
  assert_eq!(browser.focused().as_deref(), ADDRESS);
}

#[test]
fn browser_overflow_pressing_its_button_again_gives_the_focus_back() {
  let Some(mut browser) = on_used_page() else {
    return;
  };
  open_menu(&mut browser);
  press(&mut browser, "browser-overflow");
  browser.until("the menu to close", |browser| !browser.shows("browser-overflow-menu"));
  browser.pass();
  assert_eq!(browser.focused().as_deref(), PAGE);
}

/// With no filter to take the focus from, nothing is given back either.
#[test]
fn browser_overflow_gives_no_focus_where_there_was_none() {
  let Some(mut browser) = on_crowded_strip() else {
    return;
  };
  assert_eq!(browser.focused(), None);
  open_menu(&mut browser);
  assert_eq!(browser.focused().as_deref(), FILTER);
  tap(&mut browser, "Escape", "Escape");
  browser.until("the menu to close", |browser| !browser.shows("browser-overflow-menu"));
  browser.pass();
  assert_eq!(browser.focused(), None);
}

/// Focus the person moved on while the menu was open is not taken away when it closes.
#[test]
fn browser_overflow_leaves_focus_that_moved_on_while_it_was_open() {
  let Some(mut browser) = on_used_page() else {
    return;
  };
  open_menu(&mut browser);
  browser.command("l");
  browser.pass();
  assert_eq!(browser.focused().as_deref(), ADDRESS);
  tap(&mut browser, "Escape", "Escape");
  browser.until("the menu to close", |browser| !browser.shows("browser-overflow-menu"));
  browser.pass();
  assert_eq!(browser.focused().as_deref(), ADDRESS);
}

/// The menu also goes with its button, once the tabs fit: the strip gives the focus back then.
#[test]
fn browser_overflow_gives_the_focus_back_when_the_tabs_fit_again() {
  let Some(mut browser) = on_used_page() else {
    return;
  };
  open_menu(&mut browser);
  browser.feed(report(vec![tab("P0", TabOwner::Person, "Pull requests · GitHub")]));
  browser.until("the button to go", |browser| !browser.shows("browser-overflow"));
  browser.pass();
  assert!(!browser.shows("browser-overflow-menu"));
  assert_eq!(browser.focused().as_deref(), PAGE);
}

#[test]
fn browser_overflow_gives_a_prompts_field_its_focus_back() {
  let Some(mut browser) = on_crowded_strip() else {
    return;
  };
  browser.feed(BrowserEvent::DialogOpened {
    tab: BrowserTabId::new("P0"),
    dialog: BrowserDialog {
      kind: DialogKind::Prompt,
      message: "Your name?".to_owned(),
      default_prompt: String::new(),
    },
  });
  browser.until_shown("browser-dialog-answer");
  press(&mut browser, "browser-dialog-answer");
  browser.pass();
  assert_eq!(browser.focused().as_deref(), Some("browser-dialog-answer"));
  open_menu(&mut browser);
  assert_eq!(browser.focused().as_deref(), FILTER);
  tap(&mut browser, "Escape", "Escape");
  browser.until("the menu to close", |browser| !browser.shows("browser-overflow-menu"));
  browser.pass();
  assert_eq!(browser.focused().as_deref(), Some("browser-dialog-answer"));
}
