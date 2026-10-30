//! The Browser window headless, on a session whose project's browser is fed as its runner would report it: its tree is
//! mounted beside the main window's, on the same app, as lurq mounts a secondary window. A state that needs the browser
//! found (every one but *Browser not found*) is checked only where this computer has Chrome, Edge, Brave or Chromium,
//! and says so where it has none.

use std::{
  path::PathBuf,
  thread,
  time::{Duration, Instant},
};

use lurq::{
  app::{Tree, events::MouseButton},
  core::ElementRect,
  node::element::ElementRef,
};
use orchester_app::{
  browser::{BrowserLink, BrowserRoot},
  test_support::Window,
  ui::loader,
};
use orchester_core::id::RunId;
use orchester_ipc::browser::{BrowserEvent, BrowserMode, BrowserState, BrowserTab, BrowserTabId, TabOwner};
use orchester_workspace::browser::BrowserWindow;

/// The main window and the Browser window's tree on its session, showing Kontur.
pub struct Browser {
  pub window: Window,
  pub tree: Tree,
  pub root: PathBuf,
  pub state: BrowserWindow,
  pub link: BrowserLink,
}

impl Browser {
  /// The window, on a session that finds no browser when `nowhere`.
  pub fn open(nowhere: bool) -> Self {
    Self::open_at(nowhere, 1.0)
  }

  /// The window 1280 × 800 logical pixels big on a display of `scale` physical pixels to a logical one, whose events
  /// lurq takes in physical pixels.
  pub fn open_at(nowhere: bool, scale: f32) -> Self {
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
      link: link.clone(),
    };
    browser.tree.set_scale_factor(scale);
    browser.tree.resize((1280.0 * scale) as u32, (800.0 * scale) as u32);
    loader::install(&mut browser.tree);
    browser.tree.mount_root::<BrowserRoot>(&mut browser.window.app, link);
    browser.pass();
    browser
  }

  pub fn pass(&mut self) {
    self.tree.pass_headless(&mut self.window.app);
  }

  pub fn feed(&self, event: BrowserEvent) {
    self.state.receive_for_test(&self.root, "Kontur", event);
  }

  /// Delivers what the window's futures finished until `id` shows; panics after a few seconds.
  pub fn until_shown(&mut self, id: &str) {
    self.until(id, |browser| browser.shows(id));
  }

  /// Delivers what the window's futures finished until `done`; panics after a few seconds, saying `what` it waited for.
  pub fn until(&mut self, what: &str, done: impl Fn(&mut Self) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
      self.tree.tick_futures();
      self.pass();
      if done(self) {
        return;
      }
      assert!(Instant::now() < deadline, "timed out waiting for {what}");
      thread::sleep(Duration::from_millis(5));
    }
  }

  /// The texts inside `id` on one line.
  pub fn text(&mut self, id: &str) -> String {
    let element = self
      .tree
      .get_element_by_id(id)
      .unwrap_or_else(|| panic!("{id} is shown"));
    let mut all = Vec::new();
    collect(element, &mut all);
    all.join(" ")
  }

  pub fn shows(&mut self, id: &str) -> bool {
    self.tree.get_element_by_id(id).is_some()
  }

  /// Where `id` is laid out in the window, in logical pixels.
  pub fn bounds(&mut self, id: &str) -> ElementRect {
    self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  }

  /// Where `id` is laid out from the page's top left, and its size.
  pub fn on_page(&mut self, id: &str) -> (f32, f32, f32, f32) {
    let (page, element) = (self.bounds("browser-page"), self.bounds(id));
    (element.x - page.x, element.y - page.y, element.width, element.height)
  }

  /// A press on the page at its bottom right, away from what is drawn over it, then a key there.
  pub fn use_page(&mut self) {
    let page = self.bounds("browser-page");
    let (x, y) = (page.x + page.width - 10.0, page.y + page.height - 10.0);
    self.tree.mouse_down(x, y, MouseButton::Left);
    self.tree.mouse_up(x, y, MouseButton::Left);
    self.pass();
    self
      .tree
      .key_down("a".to_owned(), "KeyA".to_owned(), false, false, false);
    self.tree.key_up("a".to_owned(), "KeyA".to_owned(), false, false, false);
    self.pass();
  }

  pub fn focused(&self) -> Option<String> {
    self
      .tree
      .focused_element()
      .and_then(|element| element.id().map(str::to_owned))
  }

  /// Presses `key` with the platform's command key (Ctrl, Cmd on macOS).
  pub fn command(&mut self, key: &str) {
    let (ctrl, meta) = (!cfg!(target_os = "macos"), cfg!(target_os = "macos"));
    let code = format!("Key{}", key.to_uppercase());
    self
      .tree
      .key_down_with_meta(key.to_owned(), code.clone(), false, ctrl, false, meta);
    self
      .tree
      .key_up_with_meta(key.to_owned(), code, false, ctrl, false, meta);
    self.pass();
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
pub fn browser_installed() -> bool {
  match orchester_exec::browser::find() {
    Ok(_) => true,
    Err(unavailable) => {
      eprintln!("skipped, no browser to test with: {unavailable}");
      false
    }
  }
}

pub fn tab(id: &str, owner: TabOwner, title: &str) -> BrowserTab {
  BrowserTab {
    id: BrowserTabId::new(id),
    owner,
    title: title.to_owned(),
    url: format!("http://localhost:5173/{id}"),
    loading: false,
  }
}

/// An agent's two tabs and the person's own, `shown` shown.
pub fn state(shown: &str, mode: BrowserMode) -> (BrowserEvent, RunId) {
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
