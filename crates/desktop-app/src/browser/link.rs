//! How the main window opens and points the Browser window (`docs/design/browser.md`, "The *Browser* window"): the
//! app menu's *Browser*, the status bar's item and the run line's *Show* ask for a project's browser, a run's tab too,
//! and the window opens if it is closed or switches to it if it is open; and it opens by itself when an agent starts
//! browsing while the person lets it (decision 14, [`AutoOpen`]). lurq has no command to bring an open window to the
//! front, so a window already open switches without taking the focus.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::app::{
  WindowOpener, WindowOptions,
  component::{Component, DevtoolsInspectable},
  ctx::{Ctx, StreamEmitter},
};
use orchester_ipc::browser::BrowserTabId;
use parking_lot::Mutex;
use tokio::sync::watch;

use super::{WINDOW_HEIGHT, WINDOW_NAME, WINDOW_WIDTH, root::BrowserRoot};
use crate::{navigation::Navigation, routes::Route, services::Services, ui::loader};

/// A request to show a project's browser: its tab `tab` when given, else the tab it showed last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Show {
  pub project: PathBuf,
  pub tab: Option<BrowserTabId>,
  /// Counts the requests, so asking for the same tab twice shows it twice.
  pub sequence: u64,
}

impl DevtoolsInspectable for Show {}

/// The Browser window as the main window and the window itself reach it. Clones share it.
#[derive(Clone)]
pub struct BrowserLink {
  inner: Arc<Inner>,
}

struct Inner {
  services: Services,
  opener: WindowOpener,
  /// The main window's, where *Open Settings › Browser* goes.
  navigation: Navigation,
  /// The OS window title the window opens with, before it names its project (*Browser*).
  title: Arc<str>,
  /// Whether the window is open.
  open: Mutex<bool>,
  asked: watch::Sender<Option<Show>>,
  #[cfg(feature = "test-support")]
  minimized_for_test: lurq::core::Signal<bool>,
}

/// One link for the session's life.
impl PartialEq for BrowserLink {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl DevtoolsInspectable for BrowserLink {}

impl BrowserLink {
  /// The link of a session's window: `services`, the app's `opener`, the window's `navigation`, and the `title` the
  /// Browser window opens with.
  pub fn new(services: Services, opener: WindowOpener, navigation: Navigation, title: Arc<str>) -> Self {
    Self {
      inner: Arc::new(Inner {
        services,
        opener,
        navigation,
        title,
        open: Mutex::new(false),
        asked: watch::Sender::new(None),
        #[cfg(feature = "test-support")]
        minimized_for_test: lurq::core::Signal::new(false),
      }),
    }
  }

  /// The link the session's window provides.
  pub fn of(ctx: &mut Ctx) -> Option<Self> {
    ctx.use_context::<Self>()
  }

  pub fn services(&self) -> &Services {
    &self.inner.services
  }

  /// Settings › Browser in the main window, where the browser program is chosen.
  pub fn open_settings(&self) {
    self.inner.navigation.go(Route::Browser);
  }

  /// Shows the browser of the project at `project`, its `tab` when given, opening the window if it is closed.
  pub fn show(&self, project: &Path, tab: Option<BrowserTabId>) {
    self.inner.services.runners.browser_window().showing(project);
    self.inner.asked.send_modify(|asked| {
      let sequence = asked.as_ref().map_or(0, |asked| asked.sequence + 1);
      *asked = Some(Show {
        project: project.to_path_buf(),
        tab,
        sequence,
      });
    });
    let mut open = self.inner.open.lock();
    if *open {
      return;
    }
    *open = true;
    tracing::info!(project = %project.display(), "the Browser window opens");
    let link = self.clone();
    self.inner.opener.open_with(
      WindowOptions::new(self.inner.title.to_string(), WINDOW_WIDTH, WINDOW_HEIGHT)
        .undecorated()
        .window_name(WINDOW_NAME),
      move |app, tree| {
        loader::install(tree);
        tree.mount_root::<BrowserRoot>(app, link);
      },
    );
  }

  /// The latest request, whenever one is made.
  pub fn asked(&self) -> watch::Receiver<Option<Show>> {
    self.inner.asked.subscribe()
  }

  /// Whether the window is open.
  pub fn is_open(&self) -> bool {
    *self.inner.open.lock()
  }

  /// The window closed: the next request opens it again.
  pub(super) fn closed(&self) {
    *self.inner.open.lock() = false;
    self.inner.services.runners.browser_window().closed();
  }

  /// Whether a test minimised the window, read as lurq's report of it is.
  #[cfg(feature = "test-support")]
  pub(super) fn minimized_for_test(&self) -> bool {
    self.inner.minimized_for_test.get()
  }

  #[cfg(not(feature = "test-support"))]
  pub(super) const fn minimized_for_test(&self) -> bool {
    false
  }

  /// Minimises or restores the window for tests of what follows: only lurq's shell reports that, which a headless
  /// window has none of.
  #[cfg(feature = "test-support")]
  #[doc(hidden)]
  pub fn minimize_for_test(&self, minimized: bool) {
    self.inner.minimized_for_test.set(minimized);
  }
}

/// Opens the window by itself when an agent starts browsing while the person lets it (decision 14). Draws nothing.
pub struct AutoOpen {
  /// The start of browsing last acted on.
  handled: Mutex<u64>,
}

impl Component for AutoOpen {
  type Props = BrowserLink;

  fn create(_ctx: &mut Ctx) -> Self {
    Self { handled: Mutex::new(0) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<lurq::node::Element> {
    let link = ctx.props::<BrowserLink>().clone();
    let follow = link.clone();
    let opens = ctx
      .stream((), move |(), emitter: StreamEmitter<Opening, ()>| {
        let link = follow.clone();
        async move { follow_starts(&link, &emitter).await }
      })
      .state()
      .get()
      .data;
    if let Some(Opening { sequence, project }) = opens {
      let mut handled = self.handled.lock();
      if *handled < sequence {
        *handled = sequence;
        link.show(&project, None);
      }
    }
    lurq::node::Element::new()
  }
}

/// A start of browsing that opens the window.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Opening {
  sequence: u64,
  project: PathBuf,
}

impl DevtoolsInspectable for Opening {}

/// Emits each start of browsing after this one that opens the window, the setting read off the UI thread.
async fn follow_starts(link: &BrowserLink, emitter: &StreamEmitter<Opening, ()>) {
  let window = link.services().runners.browser_window();
  let mut started = window.started();
  started.borrow_and_update();
  let mut sequence = 0;
  while started.changed().await.is_ok() {
    let Some(start) = started.borrow_and_update().clone() else {
      continue;
    };
    let (window, workspace) = (window.clone(), link.services().workspace.clone());
    let decided = tokio::task::spawn_blocking(move || window.opens_for(&start, &workspace).map(|opens| (opens, start)));
    match decided.await {
      Ok(Ok((true, start))) => {
        sequence += 1;
        if !emitter.emit(Opening {
          sequence,
          project: start.project,
        }) {
          return;
        }
      }
      Ok(Ok((false, _))) => {}
      Ok(Err(error)) => tracing::error!(%error, "cannot read whether the Browser window opens by itself"),
      Err(error) => tracing::error!(%error, "reading whether the Browser window opens by itself failed"),
    }
  }
}
