//! The installed app's MCP server at its fixed address (`docs/design/app-mcp-connection.md`): it listens on the
//! port its settings hold across restarts, says a taken port is taken and takes no other, turns off and on, keeps one
//! token in the secret store (here one in memory) and replaces it in place, refuses a `Host` that is not its
//! loopback address, lists the clients that connected as they name themselves, and refuses screenshots and the
//! window's text while the token shows. Each test keeps a data folder of its own and a port the OS just gave.

#[path = "../mcp_server/http.rs"]
mod http;
#[path = "../support/mod.rs"]
mod support;
mod vault;

use std::{net::TcpListener, sync::Arc};

use lurq::app::{App, Tree};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  i18n,
  mcp::{self, AppMcp, ControlError, McpMode, McpSettings, ServerState, Taken, WindowLink},
  startup::Launch,
  theme::{self, Mode},
};
use orchester_core::{app_mcp::McpPort, secret::VaultError};
use orchester_workspace::{secrets::Vault, workspace::Workspace};
use serde_json::{Value, json};

use self::{
  http::{Response, delete, post, post_as},
  support::Data,
  vault::MemoryVault,
};

/// A window's root tree hosting the installed app's server, on a data folder of its own.
struct Installed {
  _app: App,
  _tree: Tree,
  link: WindowLink,
  server: AppMcp,
  vault: Arc<MemoryVault>,
  workspace: Workspace,
  launch: Launch,
  data: Data,
}

impl Installed {
  /// The server of a new installation whose port is `port`.
  fn start(port: u16) -> Self {
    Self::start_with(Data::new(), Arc::new(MemoryVault::default()), Some(port))
  }

  /// The server of the installation in `data`, its port set to `port` first if one is given.
  fn start_with(data: Data, vault: Arc<MemoryVault>, port: Option<u16>) -> Self {
    let launch = data.launch();
    let workspace = launch.workspace().expect("a session").clone();
    if let Some(port) = port {
      workspace
        .set_mcp_port(McpPort::new(u32::from(port)).expect("a port the OS gives is 1024 or more"))
        .expect("the port is kept");
    }
    let mut app = App::new();
    app.set_tokio_handle(data.runtime.handle().clone());
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    let link = WindowLink::default();
    let settings = McpSettings::with_window_control(false);
    let appearance = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
    let config = mcp::config(settings, &link, &appearance, None, app.i18n());
    let mode = McpMode::Installation {
      workspace: workspace.clone(),
      vault: Vault::with(vault.clone()),
    };
    let server = AppMcp::start(
      &mut tree,
      data.runtime.handle(),
      mode,
      (config, settings),
      &link.input_gate(),
    );
    Self {
      _app: app,
      _tree: tree,
      link,
      server,
      vault,
      workspace,
      launch,
      data,
    }
  }

  /// The same installation started again, as after the app restarts.
  fn restart(self) -> Self {
    self.server.shutdown();
    let Self {
      data, vault, launch, ..
    } = self;
    launch.end();
    Self::start_with(data, vault, None)
  }

  fn token(&self) -> String {
    self.server.token().expect("a token is in use").expose().to_owned()
  }

  fn port(&self) -> u16 {
    self.server.port().expect("the server listens").get()
  }
}

/// A port nothing listens on now.
fn free_port() -> u16 {
  TcpListener::bind(("127.0.0.1", 0))
    .and_then(|listener| listener.local_addr())
    .expect("the OS gives a port")
    .port()
}

/// An `initialize` request, which the server answers with a new session once the request is admitted.
fn knock() -> Value {
  json!({
    "jsonrpc": "2.0", "id": 1, "method": "initialize",
    "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "knock", "version": "1" } },
  })
}

/// The status of [`knock`] with `token` on `port`: 200 once admitted.
fn knocks(port: u16, token: &str) -> u16 {
  post(port, &[("Authorization", &bearer(token))], &knock()).status
}

fn bearer(token: &str) -> String {
  format!("Bearer {token}")
}

/// Opens a session as `name` `version`; its id.
fn initialize(port: u16, token: &str, (name, version): (&str, &str)) -> String {
  let initialize = json!({
    "jsonrpc": "2.0", "id": 1, "method": "initialize",
    "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": name, "version": version } },
  });
  let response = post(port, &[("Authorization", &bearer(token))], &initialize);
  assert_eq!(response.status, 200, "{}", response.text);
  response.session.expect("the server opens a session")
}

fn call(port: u16, token: &str, session: &str, tool: &str) -> Response {
  let body = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": tool, "arguments": {} } });
  post(
    port,
    &[("Authorization", &bearer(token)), ("Mcp-Session-Id", session)],
    &body,
  )
}

mod address;
mod clients;
mod mode;
mod race;
mod token;
