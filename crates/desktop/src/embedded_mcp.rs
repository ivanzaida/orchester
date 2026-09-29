//! The embedded MCP server through which agents drive and inspect the running app. It is part of
//! every build. lurq binds it to `127.0.0.1` on a port the OS picks (never a fixed one), requires
//! a bearer token on every request and writes that token, with the port, to a discovery file of
//! this process that only the user can read (`lurq/mcp/<pid>.json` in the local app data folder).
//! The token stays inside lurq: [`McpStatus`] carries the port only, so the status bar can never
//! show, copy or log it.

use std::num::NonZeroU16;

use lurq::{
  app::{Tree, ctx::Ctx},
  mcp::{McpConfig, McpHandle},
};

/// Name announced in the discovery file and to MCP clients.
const APP_NAME: &str = "orchester";
/// The only address lurq's server listens on.
const LOOPBACK: &str = "127.0.0.1";

/// Whether the embedded MCP server is listening, decided once at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpStatus {
  Listening {
    port: NonZeroU16,
  },
  /// The server did not start; lurq logs why.
  Unavailable,
}

impl lurq::app::component::DevtoolsInspectable for McpStatus {}

impl McpStatus {
  /// Starts the server on `tree`, the window's root tree.
  pub fn start(tree: &mut Tree) -> Self {
    let status = Self::of(&tree.enable_mcp(McpConfig::new().app_name(APP_NAME)));
    match status {
      Self::Listening { port } => tracing::info!("MCP server listening on {}", endpoint(port)),
      Self::Unavailable => tracing::error!("the MCP server did not start; agents cannot drive this window"),
    }
    status
  }

  /// The status of a server lurq was asked to start. lurq does not return an error when it
  /// cannot start one: it logs the cause and hands back a handle on port 0.
  pub fn of(handle: &McpHandle) -> Self {
    NonZeroU16::new(handle.port()).map_or(Self::Unavailable, |port| Self::Listening { port })
  }

  /// The status the root provides to the status bar.
  pub fn of_ctx(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the root provides the MCP status before mounting screens")
  }
}

/// `127.0.0.1:<port>`, where the server listens.
pub fn address(port: NonZeroU16) -> String {
  format!("{LOOPBACK}:{port}")
}

/// `http://127.0.0.1:<port>/mcp`, the endpoint clients connect to.
pub fn endpoint(port: NonZeroU16) -> String {
  format!("http://{}/mcp", address(port))
}
