//! An MCP client in process, over the window's production tools and input gate, for what automation sees.

#![allow(dead_code, reason = "shared by several test binaries, each of which uses a part")]

use std::{sync::Arc, thread, time::Duration};

use lurq::mcp::{
  McpHandle,
  rmcp::{RoleClient, ServiceExt, model::CallToolRequestParams, service::RunningService},
};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  mcp::{self, McpSettings, input_gate::InputGate},
};
use serde_json::Value;

use super::support::Window;

/// A tool call under way, which [`Client::answers`] waits for.
pub type Call = tokio::task::JoinHandle<String>;

pub struct Client {
  tools: RunningService<RoleClient, ()>,
  server: tokio::task::JoinHandle<()>,
  /// The window's tools and gate, for another session over them ([`Self::session`]).
  served: (McpHandle, InputGate),
}

impl Drop for Client {
  fn drop(&mut self) {
    self.server.abort();
  }
}

impl Client {
  pub fn new(window: &mut Window) -> Self {
    let appearance = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
    let config = mcp::config(
      McpSettings::with_window_control(true),
      &window.link,
      &appearance,
      window.launch.services(),
      window.app.i18n(),
    );
    let tools = window.tree.enable_mcp(config.hosted());
    let gate = window.link.input_gate();
    gate.attach(tools.clone());
    Self::over(window, (tools, gate))
  }

  /// A second client, in its own session over the same tools and gate, as another MCP client of the app is.
  pub fn session(&self, window: &mut Window) -> Self {
    Self::over(window, self.served.clone())
  }

  fn over(window: &mut Window, (tools, gate): (McpHandle, InputGate)) -> Self {
    let served = (tools.clone(), gate.clone());
    let (client, server) = tokio::io::duplex(64 * 1024);
    let server = window.data.runtime.spawn(async move {
      gate
        .guard(tools.service())
        .serve(server)
        .await
        .expect("server initializes")
        .waiting()
        .await
        .expect("server ends");
    });
    let tools = window
      .data
      .runtime
      .block_on(().serve(client))
      .expect("client initializes");
    Self { tools, server, served }
  }

  /// What the tool `name` answers, as text.
  pub fn call(&self, window: &mut Window, name: &str, arguments: &Value) -> String {
    let call = self.start(window, name, arguments);
    Self::answers(window, vec![call]).remove(0)
  }

  /// Sends the tool call `name` without waiting for its answer.
  pub fn start(&self, window: &Window, name: &str, arguments: &Value) -> Call {
    let peer = self.tools.peer().clone();
    let params =
      CallToolRequestParams::new(name.to_owned()).with_arguments(arguments.as_object().expect("arguments").clone());
    window.data.runtime.spawn(async move {
      let result = peer.call_tool(params).await.expect("tool answers");
      serde_json::to_value(result).expect("tool result").to_string()
    })
  }

  /// What `calls` answer, in their order, the window running meanwhile.
  pub fn answers(window: &mut Window, calls: Vec<Call>) -> Vec<String> {
    while !calls.iter().all(tokio::task::JoinHandle::is_finished) {
      window.tree.drain_mcp_requests(&mut window.app);
      window.tree.tick_futures();
      window.pass();
      thread::sleep(Duration::from_millis(5));
    }
    calls
      .into_iter()
      .map(|call| window.data.runtime.block_on(call).expect("call finished"))
      .collect()
  }
}
