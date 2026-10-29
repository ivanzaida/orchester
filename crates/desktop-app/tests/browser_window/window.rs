//! The Browser window headless, on a session whose project's browser is fed as its runner would report it: its tabs
//! with their owners and the toolbar (`01`), the states drawn instead of a page (`05`, `06`, `07`), what Orchester
//! draws over the page (`02`, `03`, `04`), its frame; and the main window opening it by itself when an agent starts
//! browsing, and from its app menu. The window's own tree is mounted beside the main window's, on the same app, as lurq
//! mounts a secondary window. A state that needs the browser found (every one but *Browser not found*) is checked only
//! where this computer has Chrome, Edge, Brave or Chromium, and says so where it has none.

use std::{
  io::Cursor,
  path::PathBuf,
  thread,
  time::{Duration, Instant},
};

use image::{ImageFormat, Rgb, RgbImage};
use lurq::{app::Tree, node::element::ElementRef};
use orchester_app::{
  browser::{BrowserLink, BrowserRoot},
  test_support::Window,
  ui::loader,
};
use orchester_core::id::RunId;
use orchester_ipc::browser::{
  AgentAction, BrowserDialog, BrowserEvent, BrowserMode, BrowserState, BrowserTab, BrowserTabId, DialogKind,
  ElementBox, ScreencastFrame, SelectId, SelectList, SelectOption, TabOwner,
};
use orchester_workspace::browser::BrowserWindow;

/// The main window and the Browser window's tree on its session, showing Kontur.
struct Browser {
  window: Window,
  tree: Tree,
  root: PathBuf,
  state: BrowserWindow,
}

impl Browser {
  /// The window, on a session that finds no browser when `nowhere`.
  fn open(nowhere: bool) -> Self {
    let window = Window::open();
    let services = window.launch.services().expect("the session started").clone();
    if nowhere {
      services.browser.look_nowhere();
    }
    let root = window.data.repo("repo-kontur");
    let state = services.runners.browser_window();
    let title = window.app.i18n().t("browser.window.titleAlone");
    let link = BrowserLink::new(services, window.app.window_opener(), window.navigation(), title);
    link.show(&root, None);
    let mut browser = Self {
      tree: Tree::new(),
      window,
      root,
      state,
    };
    browser.tree.resize(1280, 800);
    loader::install(&mut browser.tree);
    browser.tree.mount_root::<BrowserRoot>(&mut browser.window.app, link);
    browser.pass();
    browser
  }

  fn pass(&mut self) {
    self.tree.pass_headless(&mut self.window.app);
  }

  fn feed(&self, event: BrowserEvent) {
    self.state.receive_for_test(&self.root, "Kontur", event);
  }

  /// Delivers what the window's futures finished until `id` shows; panics after a few seconds.
  fn until_shown(&mut self, id: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
      self.tree.tick_futures();
      self.pass();
      if self.tree.get_element_by_id(id).is_some() {
        return;
      }
      assert!(Instant::now() < deadline, "timed out waiting for {id}");
      thread::sleep(Duration::from_millis(5));
    }
  }

  /// The texts inside `id` on one line.
  fn text(&mut self, id: &str) -> String {
    let element = self
      .tree
      .get_element_by_id(id)
      .unwrap_or_else(|| panic!("{id} is shown"));
    let mut all = Vec::new();
    collect(element, &mut all);
    all.join(" ")
  }

  fn shows(&mut self, id: &str) -> bool {
    self.tree.get_element_by_id(id).is_some()
  }
}

fn collect(element: ElementRef<'_>, into: &mut Vec<String>) {
  into.extend(
    element
      .text_content()
      .filter(|text| !text.chars().all(|glyph| ('\u{e000}'..='\u{f8ff}').contains(&glyph)))
      .map(str::to_owned),
  );
  for child in element.children() {
    collect(child, into);
  }
}

/// Whether this computer has a browser for the window; where it has none, the test says so and checks nothing.
#[allow(
  clippy::print_stderr,
  reason = "says that nothing was checked where no browser is installed"
)]
fn browser_installed() -> bool {
  match orchester_exec::browser::find() {
    Ok(_) => true,
    Err(unavailable) => {
      eprintln!("skipped, no browser to test with: {unavailable}");
      false
    }
  }
}

fn tab(id: &str, owner: TabOwner, title: &str) -> BrowserTab {
  BrowserTab {
    id: BrowserTabId::new(id),
    owner,
    title: title.to_owned(),
    url: format!("http://localhost:5173/{id}"),
    loading: false,
  }
}

/// An agent's two tabs and the person's own, `shown` shown.
fn state(shown: &str, mode: BrowserMode) -> (BrowserEvent, RunId) {
  let run = RunId::generate();
  let tabs = vec![
    tab("T1", TabOwner::Run { run }, "orders-sync · Kontur Admin"),
    tab("T2", TabOwner::Run { run }, "Client in reqwest - Rust"),
    tab("P1", TabOwner::Person, "Pull requests · GitHub"),
  ];
  let event = BrowserEvent::Changed(BrowserState {
    tabs,
    shown: Some(BrowserTabId::new(shown)),
    mode,
  });
  (event, run)
}

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
  window.press("app-menu");
  window.until("the app menu lists Browser", |window| {
    window.texts("menu-browser").is_some()
  });
}
