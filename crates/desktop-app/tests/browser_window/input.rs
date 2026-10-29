//! The person's input on the page as the window sends it to the tab shown (`docs/design/browser.md`, "The person's
//! input"): the window's requests are kept instead of sent to a runner, so neither a runner nor a browser runs. The
//! display has two physical pixels to a logical one, as a Retina display does, and the page lies under the toolbar and
//! an agent's activity line, so where the page was pressed is found in its CSS pixels from its own top left.

use std::{
  io::Cursor,
  thread,
  time::{Duration, Instant},
};

use image::{ImageFormat, Rgb, RgbImage};
use lurq::{
  app::events::{ImeEvent, MouseButton as Button, ScrollPhase},
  core::ElementRect,
};
use orchester_ipc::browser::{
  AgentAction, BrowserCommand, BrowserEvent, BrowserMode, BrowserTabId, KeyAction, Modifiers, MouseAction, MouseButton,
  PersonInput, ScreencastFrame,
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

/// Waits for the window to send `count` of the person's inputs, which it sends off the UI thread; each with its tab.
fn sent(browser: &Browser, count: usize) -> Vec<(BrowserTabId, PersonInput)> {
  let deadline = Instant::now() + Duration::from_secs(10);
  let (mut inputs, mut others) = (Vec::new(), Vec::new());
  while inputs.len() < count {
    assert!(
      Instant::now() < deadline,
      "timed out waiting for {count} inputs, sent {inputs:?} and {others:?}"
    );
    thread::sleep(Duration::from_millis(5));
    for (root, command) in browser.state.recorded_commands_for_test() {
      assert_eq!(root, browser.root, "the request goes to the project shown");
      match command {
        BrowserCommand::Input { tab, input } => inputs.push((tab, input)),
        other => others.push(other),
      }
    }
  }
  inputs
}

/// The only input sent next, to `tab`.
fn one_sent(browser: &Browser, tab: &str) -> PersonInput {
  let mut inputs = sent(browser, 1);
  let (to, input) = inputs.remove(0);
  assert_eq!(to, BrowserTabId::new(tab), "{input:?} goes to the tab shown");
  assert!(inputs.is_empty(), "one input, not {inputs:?}");
  input
}

/// A press and a release at `(x, y)`, which give the page the focus; what they sent is taken.
fn click(browser: &mut Browser, (x, y): (f32, f32), tab: &str) {
  browser.tree.mouse_down(x, y, Button::Left);
  browser.tree.mouse_up(x, y, Button::Left);
  sent(browser, 2)
    .iter()
    .for_each(|(to, input)| assert_eq!(to, &BrowserTabId::new(tab), "{input:?}"));
  browser.pass();
  assert_eq!(
    browser.focused().as_deref(),
    Some("browser-page-text"),
    "a press gives the page the focus"
  );
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
