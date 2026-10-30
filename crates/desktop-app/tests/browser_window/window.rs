//! The Browser window headless (`support`): its tabs with their owners and the toolbar (`01`), the states drawn instead
//! of a page (`05`, `06`, `07`), what Orchester draws over the page (`02`, `03`, `04`), its frame, its shortcuts and
//! its tab strip's overflow (`01d`, `01e`); and the main window opening it by itself when an agent starts browsing, and
//! from its app menu.

use std::io::Cursor;

use image::{ImageFormat, Rgb, RgbImage};
use lurq::app::events::{MouseButton, ScrollPhase};
use orchester_app::{
  browser::keys::{self, BrowserShortcut},
  navigation::shortcuts::Keys,
  test_support::Window,
};
use orchester_core::id::RunId;
use orchester_ipc::browser::{
  AgentAction, BrowserCommand, BrowserDialog, BrowserEvent, BrowserMode, BrowserState, BrowserTabId, DialogKind,
  ElementBox, ScreencastFrame, SelectId, SelectList, SelectOption, TabOwner,
};

use super::support::{Browser, browser_installed, state, tab};

#[test]
fn browser_window_shows_the_tabs_with_their_owners_the_toolbar_and_the_frame() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  browser.feed(state("P1", BrowserMode::Headless).0);
  browser.until_shown("browser-tab-2");
  assert!(browser.shows("browser-tabs-divider"), "agents' tabs, then the person's");
  assert!(browser.text("browser-tab-2").contains("Pull requests · GitHub"));
  assert_eq!(browser.text("browser-address-owner"), "Your tab");
  assert!(browser.text("browser-project-switcher").contains("Kontur"));
  assert!(browser.shows("browser-new-tab") && browser.shows("browser-open-in-window"));
  assert!(
    browser.shows("browser-tab-0-dot"),
    "an agent's tab the person has not looked at is new"
  );

  let mut jpeg = Cursor::new(Vec::new());
  RgbImage::from_pixel(64, 40, Rgb([250, 250, 250]))
    .write_to(&mut jpeg, ImageFormat::Jpeg)
    .expect("encode a JPEG");
  // Frames are decoded on the runtime's blocking threads, as a runner's connection hands them over.
  let runtime = browser.window.data.runtime.handle().clone();
  let _entered = runtime.enter();
  browser.state.receive_frame_for_test(
    &browser.root,
    ScreencastFrame {
      tab: BrowserTabId::new("P1"),
      sequence: 1,
      css_width: 64,
      css_height: 40,
      jpeg: jpeg.into_inner(),
    },
  );
  browser.until_shown("browser-page-frame");
}

#[test]
fn browser_window_draws_what_frames_do_not_show() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  let (changed, run) = state("T1", BrowserMode::Headless);
  browser.feed(changed);
  browser.until_shown("browser-page");
  browser.feed(BrowserEvent::AgentActed {
    run,
    tab: BrowserTabId::new("T1"),
    action: AgentAction {
      tool: "browser_type".to_owned(),
      summary: "typed 8 in Max attempts".to_owned(),
      element: Some(ElementBox {
        x: 55,
        y: 377,
        width: 268,
        height: 34,
      }),
    },
  });
  browser.until_shown("browser-activity");
  assert!(browser.text("browser-activity").contains("typed 8 in Max attempts"));
  // Before the first frame a CSS pixel is a logical one: the ring stands 4 out of the element's box (`XczoY`), the
  // pointer 28 in from its right and 16 up from its bottom (`D7l36`).
  assert_eq!(browser.on_page("browser-agent-ring"), (51.0, 373.0, 276.0, 42.0));
  let (left, top, ..) = browser.on_page("browser-agent-pointer");
  assert_eq!((left, top), (51.0 + 276.0 - 28.0, 373.0 + 42.0 - 16.0));
  browser.use_page();
  assert!(browser.text("browser-activity").contains("You're using this tab"));
  assert!(
    !browser.shows("browser-agent-mark"),
    "the person's input holds the agent, so no mark (02b)"
  );

  browser.feed(state("P1", BrowserMode::Headless).0);
  browser.feed(BrowserEvent::DialogOpened {
    tab: BrowserTabId::new("P1"),
    dialog: BrowserDialog {
      kind: DialogKind::Confirm,
      message: "Delete orders-sync?".to_owned(),
      default_prompt: String::new(),
    },
  });
  browser.until_shown("browser-dialog-message");
  assert_eq!(browser.text("browser-dialog-message"), "Delete orders-sync?");
  assert!(browser.shows("browser-dialog-cancel") && browser.shows("browser-dialog-ok"));
  browser.feed(BrowserEvent::DialogClosed {
    tab: BrowserTabId::new("P1"),
  });
  browser.feed(BrowserEvent::SelectOpened {
    tab: BrowserTabId::new("P1"),
    list: SelectList {
      select: SelectId::new(7),
      options: ["None", "Fixed delay", "Exponential", "Exponential with jitter"]
        .into_iter()
        .map(|label| SelectOption {
          label: label.to_owned(),
          selected: label == "Exponential",
          disabled: false,
        })
        .collect(),
      element: ElementBox {
        x: 55,
        y: 303,
        width: 552,
        height: 34,
      },
    },
  });
  browser.until_shown("browser-select-option-3");
  assert_eq!(browser.text("browser-select-option-3"), "Exponential with jitter");
  let (left, top, width, height) = browser.on_page("browser-select-list");
  assert_eq!(
    (left, top, width),
    (55.0, 303.0 + 34.0 + 6.0, 552.0),
    "under its field, as wide as it"
  );
  assert!(height > 4.0 * 28.0, "the list holds its four rows");
}

#[test]
fn browser_window_stands_in_for_the_page_when_there_is_none() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  browser.feed(BrowserEvent::Changed(BrowserState::default()));
  browser.until_shown("browser-empty");
  assert!(browser.text("browser-empty").contains("No pages yet"));
  browser.pass();
  assert_eq!(
    browser.focused().as_deref(),
    Some("browser-address"),
    "the empty window waits for an address"
  );
  browser.feed(state("T1", BrowserMode::Headed).0);
  browser.until_shown("browser-headed");
  assert!(
    browser
      .text("browser-headed")
      .contains("Kontur's browser is open in a window of its own")
  );
  assert!(browser.shows("browser-back-to-orchester"));
  assert!(!browser.shows("browser-open-in-window"));
}

/// The requests the window made since the last look to name a tab as shown; nothing else it asks is expected.
fn shown_asked(browser: &Browser) -> Vec<BrowserTabId> {
  browser
    .state
    .recorded_commands_for_test()
    .into_iter()
    .map(|(_, command)| match command {
      BrowserCommand::Show { tab, .. } => tab,
      other => panic!("the window asks only to show a tab here, not {other:?}"),
    })
    .collect()
}

/// `05` as a real browser reaches it: it restarted in a window of its own, so its tabs have new ids and it may report
/// none shown. The tab stays selected in the strip and the faded address line keeps its address and owner.
#[test]
fn browser_window_keeps_a_tab_shown_with_the_browser_in_a_window_of_its_own() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  browser.state.record_commands_for_test();
  let person = |id: &str| tab(id, TabOwner::Person, &format!("Page {id}"));
  browser.feed(BrowserEvent::Changed(BrowserState {
    tabs: vec![person("A"), person("B")],
    shown: Some(BrowserTabId::new("B")),
    mode: BrowserMode::Headless,
  }));
  browser.until_shown("browser-page");
  browser.until("the page area to be measured", |browser| {
    !browser.state.recorded_commands_for_test().is_empty()
  });

  browser.feed(BrowserEvent::Changed(BrowserState {
    tabs: vec![person("A2"), person("B2")],
    shown: Some(BrowserTabId::new("B2")),
    mode: BrowserMode::Headed,
  }));
  browser.until_shown("browser-headed");
  browser.until_shown("browser-tab-1-close");
  assert!(!browser.shows("browser-tab-0-close"), "only the tab shown is selected");
  assert_eq!(browser.text("browser-address-owner"), "Your tab");
  assert_eq!(address(&mut browser), "localhost:5173/B2");
  assert_eq!(browser.focused(), None);
  assert!(!browser.shows("browser-page"), "no page under the notice");

  // Named as shown, not stopped: the runner keeps that tab for *Back to Orchester*.
  assert_eq!(shown_asked(&browser), [BrowserTabId::new("B2")]);

  // A tab the person picks in the strip, once the runner reported it, is the one shown from then on.
  let center = browser.bounds("browser-tab-0").center();
  browser.tree.mouse_down(center.0, center.1, MouseButton::Left);
  browser.tree.mouse_up(center.0, center.1, MouseButton::Left);
  browser.pass();
  browser.pass();
  assert_eq!(shown_asked(&browser), [BrowserTabId::new("A2")]);
  browser.feed(BrowserEvent::Changed(BrowserState {
    tabs: vec![person("A2"), person("B2")],
    shown: Some(BrowserTabId::new("A2")),
    mode: BrowserMode::Headed,
  }));
  browser.until("the address line to follow the tab picked", |browser| {
    address(browser) == "localhost:5173/A2"
  });
  assert!(!browser.shows("browser-tab-1-close"), "it is the only one selected now");
}

/// The window opened with the browser already in a window of its own, whose runner names no tab yet: it asks for one,
/// and asks for nothing to stop.
#[test]
fn browser_window_names_a_tab_when_the_browser_in_a_window_of_its_own_reports_none() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  browser.state.record_commands_for_test();
  let tabs = vec![
    tab("A", TabOwner::Person, "Page A"),
    tab("B", TabOwner::Person, "Page B"),
  ];
  browser.feed(BrowserEvent::Changed(BrowserState {
    tabs: tabs.clone(),
    shown: None,
    mode: BrowserMode::Headed,
  }));
  browser.until_shown("browser-headed");
  browser.until("the window to name a tab", |browser| {
    shown_asked(browser) == [BrowserTabId::new("A")]
  });
  browser.feed(BrowserEvent::Changed(BrowserState {
    tabs,
    shown: Some(BrowserTabId::new("A")),
    mode: BrowserMode::Headed,
  }));
  browser.until_shown("browser-tab-0-close");
  assert_eq!(address(&mut browser), "localhost:5173/A");
  assert_eq!(browser.text("browser-address-owner"), "Your tab");
}

fn address(browser: &mut Browser) -> String {
  browser
    .tree
    .get_element_by_id_mut("browser-address")
    .and_then(|element| element.as_text_input())
    .expect("the address line is a text input")
    .value()
}

#[test]
fn browser_window_says_when_no_browser_is_found() {
  let mut browser = Browser::open(true);
  browser.feed(BrowserEvent::Changed(BrowserState::default()));
  browser.until_shown("browser-not-found");
  assert!(
    browser
      .text("browser-not-found")
      .contains("No Chromium-based browser found")
  );
  assert!(browser.shows("browser-open-settings") && browser.shows("browser-look-again"));
  assert!(
    !browser.shows("browser-open-in-window"),
    "no browser, no window to open the page in (07)"
  );
}

#[test]
fn browser_overflow_menu_lists_every_tab_in_rows_with_a_close_under_the_pointer() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  let tabs = (0..20)
    .map(|index| tab(&format!("P{index}"), TabOwner::Person, "Pull requests · GitHub"))
    .collect();
  browser.feed(BrowserEvent::Changed(BrowserState {
    tabs,
    shown: Some(BrowserTabId::new("P0")),
    mode: BrowserMode::Headless,
  }));
  browser.until_shown("browser-overflow");
  let center = |browser: &mut Browser, id: &str| {
    browser
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
      .center()
  };
  let (x, y) = center(&mut browser, "browser-overflow");
  browser.tree.mouse_down(x, y, MouseButton::Left);
  browser.tree.mouse_up(x, y, MouseButton::Left);
  browser.until_shown("browser-overflow-19");
  let row = |browser: &mut Browser, index: usize| {
    browser
      .tree
      .get_element_by_id_mut(&format!("browser-overflow-{index}"))
      .and_then(|element| element.bounds())
      .expect("the row is laid out")
  };
  let (first, second, last) = (row(&mut browser, 0), row(&mut browser, 1), row(&mut browser, 19));
  assert_eq!(
    (first.height, second.y - first.y),
    (28.0, 28.0),
    "rows of 28, one under another"
  );
  assert!(
    last.y + last.height <= 800.0,
    "all twenty rows show without scrolling, as 01d's"
  );
  assert!(!browser.shows("browser-overflow-1-close"), "× only under the pointer");
  let (x, y) = center(&mut browser, "browser-overflow-1");
  browser.tree.mouse_move(x, y);
  browser.until_shown("browser-overflow-1-close");
  assert!(!browser.shows("browser-overflow-2-close"));
}

#[test]
fn browser_tab_strip_scrolls_sideways_with_a_mouse_wheel() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  let tabs = (0..20)
    .map(|index| tab(&format!("P{index}"), TabOwner::Person, "Pull requests · GitHub"))
    .collect();
  browser.feed(BrowserEvent::Changed(BrowserState {
    tabs,
    shown: Some(BrowserTabId::new("P0")),
    mode: BrowserMode::Headless,
  }));
  browser.until_shown("browser-overflow");
  let bounds = |browser: &mut Browser, id: &str| {
    browser
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let first = bounds(&mut browser, "browser-tab-0").x;
  assert!(
    !browser.shows("browser-tabs-fade-start"),
    "the strip starts at its first tab"
  );
  let (x, y) = bounds(&mut browser, "browser-tab-1").center();
  // A mouse wheel turned down, as lurq delivers it: a vertical delta only (`01e`, the strip scrolls with the wheel).
  browser.tree.scroll(x, y, 0.0, -600.0, ScrollPhase::Scroll);
  browser.pass();
  browser.pass();
  assert!(
    bounds(&mut browser, "browser-tab-0").x < first - 100.0,
    "the tabs moved to the left"
  );
  assert!(browser.shows("browser-tabs-fade-start"), "the cut start fades (01e)");
  browser.tree.scroll(x, y, 0.0, 2000.0, ScrollPhase::Scroll);
  browser.pass();
  browser.pass();
  assert_eq!(bounds(&mut browser, "browser-tab-0").x, first, "back at the first tab");
  assert!(!browser.shows("browser-tabs-fade-start"));
}

#[test]
fn browser_window_opens_by_itself_when_an_agent_starts_browsing() {
  let mut window = Window::open();
  let services = window.launch.services().expect("the session started").clone();
  let root = window.data.repo("repo-kontur");
  let state = services.runners.browser_window();
  window.until("the main window follows starts of browsing", |_| true);
  let run = RunId::generate();
  window.until("the window opened on Kontur", |_| {
    state.receive_for_test(&root, "Kontur", BrowserEvent::AgentStartedBrowsing { run });
    state.shown_project().as_deref() == Some(root.as_path())
  });
}

#[test]
fn browser_app_menu_opens_the_window() {
  let mut window = Window::open();
  let services = window.launch.services().expect("the session started").clone();
  let root = window.data.repo("repo-kontur");
  let state = services.runners.browser_window();
  state.receive_for_test(&root, "Kontur", self::state("T1", BrowserMode::Headless).0);
  window.press("app-menu");
  window.until("the app menu lists Browser", |window| {
    window.texts("menu-browser").is_some()
  });
  assert_eq!(state.shown_project(), None, "closed until asked for");
  window.press("menu-browser");
  window.until("the window opened on Kontur, whose browser has tabs", |_| {
    state.shown_project().as_deref() == Some(root.as_path())
  });
}

/// The platform's command key with `key`: Ctrl+`key`, Cmd+`key` on macOS.
fn command(key: &'static str) -> Keys<'static> {
  Keys {
    key,
    code: "",
    ctrl: !cfg!(target_os = "macos"),
    meta: cfg!(target_os = "macos"),
    ..Keys::default()
  }
}

#[test]
fn browser_shortcuts_are_the_platforms_command_keys_and_ctrl_tab() {
  for (key, shortcut) in [
    ("t", BrowserShortcut::NewTab),
    ("w", BrowserShortcut::CloseTab),
    ("l", BrowserShortcut::Address),
    ("r", BrowserShortcut::Reload),
  ] {
    assert_eq!(keys::shortcut(command(key)), Some(shortcut), "{key}");
  }
  let ctrl_tab = Keys {
    key: "Tab",
    code: "Tab",
    ctrl: true,
    ..Keys::default()
  };
  assert_eq!(keys::shortcut(ctrl_tab), Some(BrowserShortcut::NextTab));
  assert_eq!(
    keys::shortcut(Keys {
      shift: true,
      ..ctrl_tab
    }),
    Some(BrowserShortcut::PreviousTab)
  );
  assert_eq!(
    keys::shortcut(Keys {
      key: "t",
      ..Keys::default()
    }),
    None,
    "a plain key is the page's"
  );
  assert_eq!(
    keys::shortcut(Keys {
      shift: true,
      ..command("t")
    }),
    None,
    "Shift makes another shortcut"
  );
}

#[test]
fn browser_window_shortcuts_reach_the_address_line_and_the_browser() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  browser.feed(state("P1", BrowserMode::Headless).0);
  browser.until_shown("browser-page");
  browser.command("l");
  assert_eq!(
    browser.focused().as_deref(),
    Some("browser-address"),
    "Ctrl+L puts the focus in the address line"
  );
  assert!(!browser.shows("browser-failed"));
  // No runner is connected here: the tab Ctrl+T asks the project's browser for is refused, and the window says so.
  browser.command("t");
  browser.until_shown("browser-failed");
}
