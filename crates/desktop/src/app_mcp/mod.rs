//! Orchester's own MCP server (`docs/stable-build.md`, "Orchester's MCP server"): automation, such as
//! the coordinator agent that develops Orchester through Orchester, controls projects, tasks, runs,
//! approvals and pipelines through data-level tools instead of clicking the window. Each tool goes
//! through the services the screens use ([`Services`]), so it follows the same rules, writes the same
//! records and raises the same events; a modal open in the window does not stand in its way.
//!
//! Like lurq's embedded server, which stays for looking at the window, it listens on `127.0.0.1` only,
//! on a port the OS picks, and requires a bearer token on every request. The token is written, with the
//! port, to this process's discovery file in a folder of its own ([`DiscoveryFolder`]), which only the
//! user can read and which is removed when the server stops. [`AppMcpStatus`] carries the port only.

mod discovery;
mod error;
mod handler;
mod server;
mod services;
mod tools;

use std::num::NonZeroU16;

use lurq::app::{ctx::Ctx, i18n::I18n};
use tokio::runtime::Handle;

use self::{
  discovery::DiscoveryFile,
  handler::Handler,
  server::{BearerToken, Listening},
  tools::Translate,
};
pub use self::{
  discovery::DiscoveryFolder,
  server::{address, endpoint},
  services::Services,
};

/// Whether the window starts Orchester's MCP server, and where its discovery file goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppMcpSetup {
  /// No server, e.g. for a window mounted by tests that do not use it.
  Off,
  On(DiscoveryFolder),
}

impl lurq::app::component::DevtoolsInspectable for AppMcpSetup {}

/// Whether Orchester's MCP server listens, decided when the window opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMcpStatus {
  Listening {
    port: NonZeroU16,
  },
  /// It did not start; the log says why.
  Unavailable,
}

impl lurq::app::component::DevtoolsInspectable for AppMcpStatus {}

impl AppMcpStatus {
  /// The status the root provides to the status bar.
  pub fn of_ctx(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the root provides the status of Orchester's MCP server before mounting screens")
  }
}

/// The running server and its discovery file; dropping it stops the server and removes the file.
pub struct AppMcp {
  listening: Listening,
  /// Held so the file is removed once the server stopped.
  _discovery: DiscoveryFile,
}

impl AppMcp {
  /// Starts the server for `services` on `runtime` and writes its discovery file in `folder`.
  /// `i18n` words the app's explanations in results and refusals.
  pub fn start(folder: &DiscoveryFolder, services: Services, (runtime, i18n): (&Handle, I18n)) -> Option<Self> {
    let folder = folder.path()?;
    let token = BearerToken::generate();
    let handler = Handler::new(services, Translate::new(i18n));
    let listening = match Listening::start(runtime, handler, token.clone()) {
      Ok(listening) => listening,
      Err(error) => {
        tracing::error!(%error, "Orchester's MCP server did not start");
        return None;
      }
    };
    let discovery = match DiscoveryFile::write(&folder, listening.port(), token.expose()) {
      Ok(discovery) => discovery,
      Err(error) => {
        tracing::error!(%error, folder = %folder.display(), "Orchester's MCP server wrote no discovery file");
        return None;
      }
    };
    tracing::info!(
      endpoint = %endpoint(listening.port()),
      discovery = %discovery.path().display(),
      "Orchester's MCP server listening"
    );
    Some(Self {
      listening,
      _discovery: discovery,
    })
  }

  pub fn status(&self) -> AppMcpStatus {
    AppMcpStatus::Listening {
      port: self.listening.port(),
    }
  }
}
