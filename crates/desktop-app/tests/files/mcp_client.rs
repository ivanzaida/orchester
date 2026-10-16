//! An MCP client in process, over the window's production tools and input gate, for what automation sees.
use std::{sync::Arc, thread, time::Duration};

use lurq::mcp::rmcp::{RoleClient, ServiceExt, model::CallToolRequestParams, service::RunningService};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  mcp::{self, McpSettings},
};
use serde_json::Value;

use super::support::Window;

pub struct Client {
  tools: RunningService<RoleClient, ()>,
  server: tokio::task::JoinHandle<()>,
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
    Self { tools, server }
  }

  /// What the tool `name` answers, as text.
  pub fn call(&self, window: &mut Window, name: &str, arguments: &Value) -> String {
    let peer = self.tools.peer().clone();
    let params =
      CallToolRequestParams::new(name.to_owned()).with_arguments(arguments.as_object().expect("arguments").clone());
    let request = window
      .data
      .runtime
      .spawn(async move { peer.call_tool(params).await.expect("tool answers") });
    while !request.is_finished() {
      window.tree.drain_mcp_requests(&mut window.app);
      window.tree.tick_futures();
      window.pass();
      thread::sleep(Duration::from_millis(5));
    }
    let result = window.data.runtime.block_on(request).expect("call finished");
    serde_json::to_value(result).expect("tool result").to_string()
  }
}
