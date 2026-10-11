//! The actual MCP tools redact credentials and refuse automated replacement confirmation.
//! An in-process MCP transport avoids discovery-file ACLs while using the production input gate and tools.
use std::{sync::Arc, thread, time::Duration};

use lurq::mcp::rmcp::{RoleClient, ServiceExt, model::CallToolRequestParams, service::RunningService};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  mcp::{self, McpSettings},
};
use orchester_core::secret::Environment;
use serde_json::{Value, json};

use super::harness::{NEXT, Secrets, VALUE};

struct Client {
  tools: RunningService<RoleClient, ()>,
  server: tokio::task::JoinHandle<()>,
}
impl Drop for Client {
  fn drop(&mut self) {
    self.server.abort();
  }
}
impl Client {
  fn new(s: &mut Secrets) -> Self {
    let appearance = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
    let config = mcp::config(
      McpSettings::with_window_control(true),
      &s.window.link,
      &appearance,
      s.window.launch.services(),
      s.window.app.i18n(),
    );
    let tools = s.window.tree.enable_mcp(config.hosted());
    let gate = s.window.link.input_gate();
    gate.attach(tools.clone());
    let (client, server) = tokio::io::duplex(64 * 1024);
    let server = s.window.data.runtime.spawn(async move {
      gate
        .guard(tools.service())
        .serve(server)
        .await
        .expect("server initializes")
        .waiting()
        .await
        .expect("server ends");
    });
    let tools = s
      .window
      .data
      .runtime
      .block_on(().serve(client))
      .expect("client initializes");
    Self { tools, server }
  }
  fn call(&self, s: &mut Secrets, name: &str, arguments: &Value) -> Value {
    let peer = self.tools.peer().clone();
    let params =
      CallToolRequestParams::new(name.to_owned()).with_arguments(arguments.as_object().expect("arguments").clone());
    let request = s
      .window
      .data
      .runtime
      .spawn(async move { peer.call_tool(params).await.expect("tool answers") });
    while !request.is_finished() {
      s.window.tree.drain_mcp_requests(&mut s.window.app);
      s.window.tree.tick_futures();
      s.window.pass();
      thread::sleep(Duration::from_millis(5));
    }
    let result = s.window.data.runtime.block_on(request).expect("call finished");
    serde_json::to_value(result).expect("tool result")
  }
  fn click(&self, s: &mut Secrets, id: &str) -> Value {
    let found = self.call(s, "lurq_find_by_id", &json!({"id": id}));
    let reference = found["content"][0]["text"]
      .as_str()
      .expect("ref")
      .split_whitespace()
      .next()
      .expect("element")
      .to_owned();
    self.call(s, "lurq_interact", &json!({"action": "click", "ref": reference}))
  }
}

#[test]
fn secrets_mcp_tree_masks_values_and_refuses_automated_replacement_confirmation() {
  let mut s = Secrets::new();
  let secret = s.seed("DOCS_TOKEN", Environment::Development);
  s.bind(&secret);
  s.list();
  s.click("secret-replace");
  s.set("secret-value", NEXT);
  let client = Client::new(&mut s);
  let tree = client.call(&mut s, "lurq_read_tree", &json!({}));
  assert!(!tree["isError"].as_bool().unwrap_or(false), "tree read succeeds");
  let text = tree.to_string();
  assert!(!text.contains(VALUE), "the stored value never reaches the MCP tree");
  assert!(!text.contains(NEXT), "the entered value never reaches the MCP tree");
  let asked = client.click(&mut s, "secret-save");
  assert!(
    !asked["isError"].as_bool().unwrap_or(false),
    "automation may request the question"
  );
  s.until("person-only question", |s| {
    s.exists("secret-confirm-replacement-confirm")
  });
  let refused = client.click(&mut s, "secret-confirm-replacement-confirm");
  assert!(refused["isError"].as_bool().unwrap_or(false));
  let error: Value = serde_json::from_str(refused["content"][0]["text"].as_str().expect("error")).expect("JSON error");
  assert_eq!(error["code"], "person_only");
  assert!(error["message"].as_str().expect("message").contains("Secrets page"));
  s.click("secret-confirm-replacement-keep");
  assert!(!s.window.link.input_gate().protecting());
}
