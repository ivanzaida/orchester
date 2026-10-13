use std::{sync::Arc, thread, time::Duration};

use lurq::mcp::rmcp::{RoleClient, ServiceExt, model::CallToolRequestParams, service::RunningService};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  mcp::{self, McpSettings},
};
use serde_json::{Value, json};

use super::integration_support::Page;

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
  pub fn new(page: &mut Page, window_control: bool) -> Self {
    let appearance = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
    let mut services = page.window.launch.services().expect("session").clone();
    services.vault = orchester_workspace::secrets::Vault::with(Arc::new(super::integration_support::PrivateVault));
    let config = mcp::config(
      McpSettings::with_window_control(window_control),
      &page.window.link,
      &appearance,
      Some(&services),
      page.window.app.i18n(),
    );
    let tools = page.window.tree.enable_mcp(config.hosted());
    let gate = page.window.link.input_gate();
    gate.attach(tools.clone());
    let (client, server) = tokio::io::duplex(64 * 1024);
    let server = page.window.data.runtime.spawn(async move {
      gate
        .guard(tools.service())
        .serve(server)
        .await
        .expect("private server")
        .waiting()
        .await
        .expect("server ends");
    });
    let tools = page
      .window
      .data
      .runtime
      .block_on(().serve(client))
      .expect("private client");
    Self { tools, server }
  }
  pub fn call(&self, page: &mut Page, name: &str, args: &Value) -> Value {
    let peer = self.tools.peer().clone();
    let params =
      CallToolRequestParams::new(name.to_owned()).with_arguments(args.as_object().expect("arguments").clone());
    let request = page
      .window
      .data
      .runtime
      .spawn(async move { peer.call_tool(params).await });
    let deadline = std::time::Instant::now() + Duration::from_secs(180);
    while !request.is_finished() {
      page.window.tree.drain_mcp_requests(&mut page.window.app);
      page.window.tree.tick_futures();
      page.window.pass();
      assert!(std::time::Instant::now() < deadline, "private tool timed out");
      thread::sleep(Duration::from_millis(5));
    }
    match page.window.data.runtime.block_on(request).expect("request ends") {
      Ok(result) => serde_json::to_value(result).expect("tool result"),
      Err(_) => json!({"isError":true,"rpcError":true}),
    }
  }
  pub fn reference(&self, page: &mut Page, id: &str) -> String {
    page.ready(id);
    let found = self.call(page, "lurq_find_by_id", &json!({"id":id}));
    found["content"][0]["text"]
      .as_str()
      .expect("element result")
      .split_whitespace()
      .next()
      .expect("reference")
      .to_owned()
  }
  pub fn click(&self, page: &mut Page, id: &str) -> Value {
    let reference = self.reference(page, id);
    self.call(page, "lurq_interact", &json!({"action":"click","ref":reference}))
  }
  pub fn set(&self, page: &mut Page, id: &str, value: &str) -> Value {
    let reference = self.reference(page, id);
    self.call(page, "lurq_set_value", &json!({"ref":reference,"value":value}))
  }
}
pub fn ok(result: &Value) {
  assert!(!result["isError"].as_bool().unwrap_or(false), "{result}");
}
pub fn value(result: &Value) -> Value {
  serde_json::from_str(result["content"][0]["text"].as_str().expect("result text")).expect("JSON")
}
