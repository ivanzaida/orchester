//! Settings › Automation › *Orchester MCP server* headless, on a session's window whose server runs as the installed
//! app's (a data folder of its own, a secret store in memory, a port the OS just gave): listening, a taken port, off,
//! an unreadable token, rotating the token in place and showing it, *Connect an agent*, and a QA copy's one line.
//! Copy is not pressed here: it would write the person's clipboard.

#[path = "../support/mod.rs"]
mod support;
#[path = "../mcp_stable/vault.rs"]
mod vault;

use std::{net::TcpListener, sync::Arc};

use lurq::app::{App, Tree, events::MouseButton};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  i18n,
  mcp::{self, AppMcp, McpLink, McpMode, McpSettings, WindowLink},
  root::{Root, RootProps},
  routes::Route,
  startup::Launch,
  theme::{self, Mode},
};
use orchester_core::app_mcp::McpPort;
use orchester_workspace::secrets::Vault;

use self::{support::Data, vault::MemoryVault};

mod connect;
mod person;
mod states;
mod token;

/// Where this platform keeps the token, as Settings names it.
const STORE: &str = if cfg!(target_os = "macos") {
  "your login keychain"
} else {
  "Windows Credential Manager"
};

/// The window on Settings, its server and the store its token is in.
struct Shown {
  app: App,
  tree: Tree,
  link: WindowLink,
  server: AppMcp,
  _launch: Launch,
  _data: Data,
}

impl Shown {
  /// The installed app's server on `port`, its token in `vault`.
  fn installed(port: u16, vault: Arc<MemoryVault>) -> Self {
    Self::installed_as(port, vault, true)
  }

  /// [`Shown::installed`], its server turned on or off before it first starts.
  fn installed_as(port: u16, vault: Arc<MemoryVault>, enabled: bool) -> Self {
    let data = Data::new();
    let launch = data.launch();
    let workspace = launch.workspace().expect("a session").clone();
    workspace
      .set_mcp_port(McpPort::new(u32::from(port)).expect("a port the OS gives is 1024 or more"))
      .expect("the port is kept");
    workspace.set_mcp_server_enabled(enabled).expect("kept");
    let mode = McpMode::Installation {
      workspace,
      vault: Vault::with(vault),
    };
    Self::open(data, launch, mode)
  }

  /// A QA copy, its discovery file in the data folder.
  fn qa() -> Self {
    let data = Data::new();
    let launch = data.launch();
    let mode = McpMode::Qa(data.dir.path().join("mcp"));
    Self::open(data, launch, mode)
  }

  fn open(data: Data, launch: Launch, mode: McpMode) -> Self {
    let mut app = App::new();
    app.set_tokio_handle(data.runtime.handle().clone());
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(1440, 1400);
    let link = WindowLink::default();
    let settings = McpSettings::with_window_control(false);
    let appearance = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
    let config = mcp::config(settings, &link, &appearance, launch.services(), app.i18n());
    let server = AppMcp::start(
      &mut tree,
      data.runtime.handle(),
      mode,
      (config, settings),
      &link.input_gate(),
    );
    tree.mount_root::<Root>(
      &mut app,
      RootProps {
        link: link.clone(),
        mcp: McpLink::new(&server),
        start: launch.start(),
      },
    );
    tree.pass_headless(&mut app);
    let mut shown = Self {
      app,
      tree,
      link,
      server,
      _launch: launch,
      _data: data,
    };
    shown
      .link
      .navigation()
      .expect("the window attached its navigation")
      .go(Route::Settings);
    shown.until("the MCP server's section", |shown| {
      shown.texts("settings-mcp").is_some()
    });
    shown
  }

  fn pass(&mut self) {
    self.tree.pass_headless(&mut self.app);
  }

  /// Delivers the window's futures and lays it out until `done` holds; panics with `what` after a few seconds.
  fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
      self.tree.tick_futures();
      self.pass();
      if done(self) {
        return;
      }
      assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
      std::thread::sleep(std::time::Duration::from_millis(5));
    }
  }

  /// The texts inside the element `id`, icon glyphs left out.
  fn texts(&mut self, id: &str) -> Option<Vec<String>> {
    let element = self.tree.get_element_by_id(id)?;
    let mut all = Vec::new();
    collect(element, &mut all);
    all.retain(|text| !text.chars().all(|glyph| ('\u{e000}'..='\u{f8ff}').contains(&glyph)));
    Some(all)
  }

  /// Whether the element `id` says `words`.
  fn says(&mut self, id: &str, words: &str) -> bool {
    self
      .texts(id)
      .is_some_and(|texts| texts.iter().any(|text| text == words))
  }

  fn click(&mut self, id: &str) {
    let bounds = self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    self.tree.mouse_move(x, y);
    self.tree.mouse_down(x, y, MouseButton::Left);
    self.tree.mouse_up(x, y, MouseButton::Left);
    self.pass();
  }

  /// Where the element `id` is laid out: x, y, width and height.
  fn bounds(&mut self, id: &str) -> [f32; 4] {
    let bounds = self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    [bounds.x, bounds.y, bounds.width, bounds.height]
  }

  fn token(&self) -> String {
    self.server.token().expect("a token is in use").expose().to_owned()
  }

  /// Every text of the window, to look for the token in.
  fn everything(&mut self) -> Vec<String> {
    self.texts("settings-mcp").unwrap_or_default()
  }
}

fn collect(element: lurq::node::element::ElementRef<'_>, into: &mut Vec<String>) {
  into.extend(element.text_content().map(str::to_owned));
  for child in element.children() {
    collect(child, into);
  }
}

/// A port nothing listens on now.
fn free_port() -> u16 {
  TcpListener::bind(("127.0.0.1", 0))
    .and_then(|listener| listener.local_addr())
    .expect("the OS gives a port")
    .port()
}
