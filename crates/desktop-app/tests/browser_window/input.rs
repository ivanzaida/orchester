//! The person's input on the page as the window sends it to the tab shown (`docs/design/browser.md`, "The person's
//! input"), and what Enter in the address line asks: the window's requests are kept instead of sent to a runner, so
//! neither a runner nor a browser runs. The display has two physical pixels to a logical one, as a Retina display does,
//! and the page lies under the toolbar and an agent's activity line, so where the page was pressed is found in its CSS
//! pixels from its own top left.

use std::io::Cursor;

use image::{ImageFormat, Rgb, RgbImage};
use lurq::{
  app::events::{ImeEvent, MouseButton as Button, ScrollPhase},
  core::ElementRect,
};
use orchester_ipc::browser::{
  AgentAction, BrowserCommand, BrowserEvent, BrowserMode, BrowserState, BrowserTabId, KeyAction, Modifiers,
  MouseAction, MouseButton, Navigation, PersonInput, ScreencastFrame,
};

use super::support::{Browser, browser_installed, state};

const SCALE: f32 = 2.0;

/// The window on an agent's tab `T1` that the agent just acted on, so its activity line shows over the page, with a
/// frame as big in CSS pixels as the page is in logical ones, as the runner sizes it; the page's bounds then, and the
/// activity line's.
fn on_agents_tab() -> Option<(Browser, ElementRect, ElementRect)> {
  if !browser_installed() {
    return None;
  }
  let mut browser = Browser::open_at(false, SCALE);
  browser.state.record_commands_for_test();
  let (changed, run) = state("T1", BrowserMode::Headless);
  browser.feed(changed);
  browser.feed(BrowserEvent::AgentActed {
    run,
    tab: BrowserTabId::new("T1"),
    action: AgentAction {
      tool: "browser_navigate".to_owned(),
      summary: "opened localhost:5173".to_owned(),
      element: None,
    },
  });
  browser.until_shown("browser-activity");
  browser.until_shown("browser-page");
  let (activity, page) = (browser.bounds("browser-activity"), browser.bounds("browser-page"));
  assert!(
    page.y >= activity.y + activity.height && activity.y > 0.0,
    "the page lies under the toolbar and the activity line"
  );
  frame(&browser, "T1", 1, (page.width as u32, page.height as u32));
  browser.until_shown("browser-page-frame");
  Some((browser, page, activity))
}

/// The point `(x, y)` of the page, from its top left in logical pixels, in the physical pixels lurq takes.
fn at(page: ElementRect, x: f32, y: f32) -> (f32, f32) {
  ((page.x + x) * SCALE, (page.y + y) * SCALE)
}

/// The requests the window made since the last look, to the project shown. It hands each over as it is made, so they
/// are all here by the end of the event that made them.
fn requests(browser: &Browser) -> Vec<BrowserCommand> {
  let recorded = browser.state.recorded_commands_for_test();
  recorded
    .into_iter()
    .map(|(root, command)| {
      assert_eq!(root, browser.root, "the request goes to the project shown");
      command
    })
    .collect()
}

/// The person's inputs among the requests since the last look, each with its tab.
fn sent(browser: &Browser) -> Vec<(BrowserTabId, PersonInput)> {
  requests(browser)
    .into_iter()
    .filter_map(|command| match command {
      BrowserCommand::Input { tab, input } => Some((tab, input)),
      _ => None,
    })
    .collect()
}

/// The only input sent since the last look, to `tab`.
fn one_sent(browser: &Browser, tab: &str) -> PersonInput {
  let mut inputs = sent(browser);
  assert_eq!(inputs.len(), 1, "one input, not {inputs:?}");
  let (to, input) = inputs.remove(0);
  assert_eq!(to, BrowserTabId::new(tab), "{input:?} goes to the tab shown");
  input
}

/// A press and a release at `(x, y)`, which give the page the focus; what they sent is taken.
fn click(browser: &mut Browser, (x, y): (f32, f32), tab: &str) {
  browser.tree.mouse_down(x, y, Button::Left);
  browser.tree.mouse_up(x, y, Button::Left);
  let inputs = sent(browser);
  assert_eq!(inputs.len(), 2, "a press and a release, not {inputs:?}");
  inputs
    .iter()
    .for_each(|(to, input)| assert_eq!(to, &BrowserTabId::new(tab), "{input:?}"));
  browser.pass();
  assert_eq!(
    browser.focused().as_deref(),
    Some("browser-page-text"),
    "a press gives the page the focus"
  );
}

/// With the address line focused, what the window asks when `typed` is typed there and Enter pressed.
fn enter_address(browser: &mut Browser, typed: &str) -> Vec<BrowserCommand> {
  assert_eq!(browser.focused().as_deref(), Some("browser-address"));
  browser
    .tree
    .get_element_by_id_mut("browser-address")
    .and_then(|element| element.as_text_input())
    .expect("the address line is a text input")
    .set_value(typed);
  browser.pass();
  requests(browser);
  key_down(browser, "Enter", "Enter");
  requests(browser)
}

fn mouse(action: MouseAction, (x, y): (i32, i32), button: MouseButton, clicks: u8) -> PersonInput {
  PersonInput::Mouse {
    action,
    x,
    y,
    button,
    clicks,
    modifiers: Modifiers::default(),
  }
}

fn key(action: KeyAction, (key, code): (&str, &str), text: Option<&str>) -> PersonInput {
  PersonInput::Key {
    action,
    key: key.to_owned(),
    code: code.to_owned(),
    text: text.map(str::to_owned),
    modifiers: Modifiers::default(),
  }
}

fn key_down(browser: &mut Browser, key: &str, code: &str) {
  browser
    .tree
    .key_down(key.to_owned(), code.to_owned(), false, false, false);
}

/// A frame of `tab`, `css` big in CSS pixels, handed over as a runner's connection does.
fn frame(browser: &Browser, tab: &str, sequence: u64, (css_width, css_height): (u32, u32)) {
  let mut jpeg = Cursor::new(Vec::new());
  RgbImage::from_pixel(64, 40, Rgb([250, 250, 250]))
    .write_to(&mut jpeg, ImageFormat::Jpeg)
    .expect("encode a JPEG");
  let runtime = browser.window.data.runtime.handle().clone();
  let _entered = runtime.enter();
  browser.state.receive_frame_for_test(
    &browser.root,
    ScreencastFrame {
      tab: BrowserTabId::new(tab),
      sequence,
      css_width,
      css_height,
      jpeg: jpeg.into_inner(),
    },
  );
}

/// The hidden input that holds the page's focus: its value and the composition it shows.
fn hidden(browser: &mut Browser) -> (String, Option<String>) {
  let input = browser
    .tree
    .get_element_by_id_mut("browser-page-text")
    .and_then(|element| element.as_text_input())
    .expect("the page has its hidden input");
  (input.value(), input.composition())
}

#[test]
fn browser_person_pointer_and_wheel_reach_the_tab_in_css_pixels() {
  let Some((mut browser, page, _)) = on_agents_tab() else {
    return;
  };
  let (x, y) = at(page, 100.0, 50.0);
  browser.tree.mouse_down(x, y, Button::Left);
  assert_eq!(
    one_sent(&browser, "T1"),
    mouse(MouseAction::Pressed, (100, 50), MouseButton::Left, 1)
  );
  let (x, y) = at(page, 140.0, 70.0);
  browser.tree.mouse_move(x, y);
  assert_eq!(
    one_sent(&browser, "T1"),
    mouse(MouseAction::Moved, (140, 70), MouseButton::Left, 0),
    "a move with the button held drags"
  );
  browser.tree.mouse_up(x, y, Button::Left);
  assert_eq!(
    one_sent(&browser, "T1"),
    mouse(MouseAction::Released, (140, 70), MouseButton::Left, 1)
  );
  let (x, y) = at(page, 300.0, 200.0);
  browser.tree.scroll(x, y, 0.0, -120.0, ScrollPhase::Scroll);
  assert_eq!(
    one_sent(&browser, "T1"),
    PersonInput::Wheel {
      x: 300,
      y: 200,
      delta_x: 0,
      delta_y: 120,
      modifiers: Modifiers::default(),
    },
    "the wheel turned down scrolls the page down"
  );
}

#[test]
fn browser_person_keys_and_input_method_text_reach_the_tab() {
  let Some((mut browser, page, _)) = on_agents_tab() else {
    return;
  };
  click(&mut browser, at(page, 100.0, 50.0), "T1");
  key_down(&mut browser, "ф", "KeyA");
  assert_eq!(
    one_sent(&browser, "T1"),
    key(KeyAction::Down, ("ф", "KeyA"), Some("ф")),
    "a key types its text, a Cyrillic letter too"
  );
  browser
    .tree
    .key_up("ф".to_owned(), "KeyA".to_owned(), false, false, false);
  assert_eq!(one_sent(&browser, "T1"), key(KeyAction::Up, ("ф", "KeyA"), None));
  key_down(&mut browser, "Enter", "Enter");
  assert_eq!(
    one_sent(&browser, "T1"),
    key(KeyAction::Down, ("Enter", "Enter"), None),
    "Enter on the page is the page's, not the address line's"
  );

  browser.tree.ime(ImeEvent::Enabled);
  browser.tree.ime(ImeEvent::Preedit {
    text: "にほん".to_owned(),
    cursor: Some((9, 9)),
  });
  assert_eq!(
    hidden(&mut browser),
    (String::new(), Some("にほん".to_owned())),
    "the composition under way shows only in the hidden input"
  );
  browser.tree.ime(ImeEvent::Commit("日本".to_owned()));
  assert_eq!(
    one_sent(&browser, "T1"),
    PersonInput::Text {
      text: "日本".to_owned()
    },
    "what the input method commits is sent as text, and nothing of its composition"
  );
  browser.tree.ime(ImeEvent::Commit("привет".to_owned()));
  assert_eq!(
    one_sent(&browser, "T1"),
    PersonInput::Text {
      text: "привет".to_owned()
    }
  );
  assert_eq!(
    hidden(&mut browser),
    (String::new(), None),
    "the hidden input keeps nothing"
  );
}

#[test]
fn browser_person_burst_of_inputs_is_sent_in_the_order_it_was_made() {
  let Some((mut browser, page, _)) = on_agents_tab() else {
    return;
  };
  requests(&browser);
  let mut made = Vec::new();
  for step in 0..200 {
    let (x, y) = at(page, 10.0 + step as f32, 50.0);
    browser.tree.mouse_move(x, y);
    made.push(mouse(MouseAction::Moved, (10 + step, 50), MouseButton::None, 0));
  }
  let (x, y) = at(page, 300.0, 50.0);
  browser.tree.mouse_down(x, y, Button::Left);
  browser.tree.mouse_up(x, y, Button::Left);
  made.push(mouse(MouseAction::Pressed, (300, 50), MouseButton::Left, 1));
  made.push(mouse(MouseAction::Released, (300, 50), MouseButton::Left, 1));
  let inputs: Vec<PersonInput> = sent(&browser).into_iter().map(|(_, input)| input).collect();
  assert_eq!(inputs, made);
}

/// No key event of a composition reaches the page. A key the input method passes on with a text (a space after a Korean
/// syllable) is not lost: lurq types it in the hidden input, so it reaches the page as text.
#[test]
fn browser_person_keys_of_a_composition_are_not_sent() {
  let Some((mut browser, page, _)) = on_agents_tab() else {
    return;
  };
  click(&mut browser, at(page, 100.0, 50.0), "T1");
  browser.tree.ime(ImeEvent::Enabled);
  browser.tree.ime(ImeEvent::Preedit {
    text: "に".to_owned(),
    cursor: Some((3, 3)),
  });
  for (name, code) in [("Process", "KeyA"), ("Enter", "Enter")] {
    key_down(&mut browser, name, code);
    browser
      .tree
      .key_up(name.to_owned(), code.to_owned(), false, false, false);
    assert_eq!(sent(&browser), [], "{name} is the composition's, pressed and released");
  }
  key_down(&mut browser, " ", "Space");
  assert_eq!(
    one_sent(&browser, "T1"),
    PersonInput::Text { text: " ".to_owned() },
    "a key passed on keeps its text, without a key event"
  );
  browser
    .tree
    .key_up(" ".to_owned(), "Space".to_owned(), false, false, false);
  assert_eq!(sent(&browser), [], "its release is the composition's");

  browser.tree.ime(ImeEvent::Commit("日".to_owned()));
  assert_eq!(one_sent(&browser, "T1"), PersonInput::Text { text: "日".to_owned() });
  browser
    .tree
    .key_up("Enter".to_owned(), "Enter".to_owned(), false, false, false);
  assert_eq!(
    sent(&browser),
    [],
    "the release of the key that confirmed is the composition's"
  );

  key_down(&mut browser, "b", "KeyB");
  assert_eq!(
    one_sent(&browser, "T1"),
    key(KeyAction::Down, ("b", "KeyB"), Some("b")),
    "the next key is the page's again"
  );
}

/// Input method candidates show where the hidden input stands.
#[test]
fn browser_person_press_moves_the_hidden_input_there() {
  let Some((mut browser, page, _)) = on_agents_tab() else {
    return;
  };
  click(&mut browser, at(page, 100.0, 50.0), "T1");
  assert_eq!(browser.on_page("browser-page-text"), (100.0, 50.0, 1.0, 1.0));
  click(&mut browser, at(page, 240.0, 130.0), "T1");
  assert_eq!(browser.on_page("browser-page-text"), (240.0, 130.0, 1.0, 1.0));
}

#[test]
fn browser_address_line_goes_to_the_typed_address_on_enter() {
  let Some((mut browser, ..)) = on_agents_tab() else {
    return;
  };
  browser.command("l");
  assert_eq!(
    enter_address(&mut browser, "docs.rs/reqwest"),
    [BrowserCommand::Navigate {
      tab: BrowserTabId::new("T1"),
      to: Navigation::Address {
        url: "https://docs.rs/reqwest".to_owned()
      },
    }],
    "the tab shown goes there, and the page gets no key"
  );
}

#[test]
fn browser_address_line_opens_a_tab_on_enter_when_the_window_has_none() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  browser.state.record_commands_for_test();
  browser.feed(BrowserEvent::Changed(BrowserState::default()));
  browser.until_shown("browser-empty");
  browser.pass();
  assert_eq!(
    enter_address(&mut browser, "localhost:5173"),
    [BrowserCommand::OpenTab {
      url: Some("http://localhost:5173".to_owned())
    }]
  );
}

#[test]
fn browser_person_input_follows_the_tab_shown() {
  let Some((mut browser, page, activity)) = on_agents_tab() else {
    return;
  };
  click(&mut browser, at(page, 100.0, 50.0), "T1");
  // The person picks their own tab in the strip, and the runner shows it with its frame: their input goes there now.
  let (x, y) = browser.bounds("browser-tab-2").center();
  browser.tree.mouse_down(x * SCALE, y * SCALE, Button::Left);
  browser.tree.mouse_up(x * SCALE, y * SCALE, Button::Left);
  browser.feed(state("P1", BrowserMode::Headless).0);
  browser.until("the person's tab shown", |browser| {
    browser.text("browser-address-owner") == "Your tab"
  });
  assert!(!browser.shows("browser-page-frame"), "no frame of the person's tab yet");
  let page = browser.bounds("browser-page");
  assert!(
    page.y < activity.y + activity.height,
    "without an agent's activity line the page moved up"
  );
  frame(&browser, "P1", 2, (page.width as u32, page.height as u32));
  browser.until_shown("browser-page-frame");
  let (x, y) = at(page, 10.0, 20.0);
  browser.tree.mouse_move(x, y);
  assert_eq!(
    one_sent(&browser, "P1"),
    mouse(MouseAction::Moved, (10, 20), MouseButton::None, 0),
    "the pointer comes back over the page from the strip, measured from where the page is now"
  );
  click(&mut browser, (x, y), "P1");
  key_down(&mut browser, "b", "KeyB");
  assert_eq!(one_sent(&browser, "P1"), key(KeyAction::Down, ("b", "KeyB"), Some("b")));
}
