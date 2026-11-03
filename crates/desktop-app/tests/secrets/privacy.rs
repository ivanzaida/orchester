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
  for allowed in [false, true] {
    let mut s = Secrets::new();
    s.window
      .launch
      .workspace()
      .expect("private session")
      .set_automation_changes_permissions(allowed)
      .expect("private human opt-in fixture");
    let secret = s.seed("DOCS_TOKEN", Environment::Development);
    s.bind(&secret);
    s.open(&secret);
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
    lurq::app::events::with_synthetic_input(|| s.click("secret-confirm-replacement-confirm"));
    assert!(
      s.exists("secret-confirm-replacement-confirm"),
      "non-MCP synthetic input cannot confirm Secrets"
    );
    assert!(s.vault.contains(
      &orchester_core::secret::VaultKey::new(super::support::project_id(), secret.id()),
      VALUE
    ));
    lurq::app::events::with_synthetic_input(|| s.click("secret-confirm-replacement-keep"));
    assert!(
      s.exists("secret-confirm-replacement-confirm"),
      "synthetic Keep cannot answer Secrets"
    );
    let refused = client.click(&mut s, "secret-confirm-replacement-confirm");
    assert!(refused["isError"].as_bool().unwrap_or(false));
    let error: Value =
      serde_json::from_str(refused["content"][0]["text"].as_str().expect("error")).expect("JSON error");
    assert_eq!(error["code"], "person_only");
    assert!(error["message"].as_str().expect("message").contains("Environment tab"));
    s.click("secret-confirm-replacement-keep");
    assert!(!s.window.link.input_gate().protecting());
  }
}

#[test]
fn secrets_stale_unused_replacement_rechecks_references_and_requires_person_confirmation() {
  use std::sync::atomic::Ordering;

  use orchester_core::{secret::VaultKey, store::SecretStore};

  use super::support::project_id;

  let mut s = Secrets::new();
  let secret = s.seed("DOCS_TOKEN", Environment::Development);
  let key = VaultKey::new(project_id(), secret.id());
  s.open(&secret);
  s.click("secret-replace");
  s.set("secret-value", NEXT);
  assert!(!s.text("secret-uses").contains("kontur-docs"));
  // The page keeps its unused snapshot while another caller stores a binding.
  s.bind(&secret);
  let client = Client::new(&mut s);
  let requested = client.click(&mut s, "secret-save");
  assert!(!requested["isError"].as_bool().unwrap_or(false));
  s.until("save-time replacement question", |s| {
    s.exists("secret-confirm-replacement-confirm")
  });
  assert!(s.window.link.input_gate().protecting());
  assert!(s.vault.contains(&key, VALUE));
  assert_eq!(s.vault.stores.load(Ordering::SeqCst), 1);
  assert!(
    s.db()
      .secret(secret.id())
      .expect("metadata")
      .expect("secret")
      .rotated_at()
      .is_none()
  );
  s.private();
  let refused = client.click(&mut s, "secret-confirm-replacement-confirm");
  assert!(refused["isError"].as_bool().unwrap_or(false));
  let error: Value = serde_json::from_str(refused["content"][0]["text"].as_str().expect("error")).expect("JSON error");
  assert_eq!(error["code"], "person_only");
  assert!(s.vault.contains(&key, VALUE));
  s.click("secret-confirm-replacement-keep");
  assert!(!s.window.link.input_gate().protecting());
  assert!(s.vault.contains(&key, VALUE));
  s.click("secret-save");
  s.until("armed replacement question", |s| {
    s.window.tree.tick_futures();
    s.exists("secret-confirm-replacement-confirm")
  });
  s.window.pass();
  s.click("secret-confirm-replacement-confirm");
  s.until("person replaced", |s| {
    s.exists("secret-inspector-name") && !s.exists("secret-value")
  });
  assert!(s.vault.contains(&key, NEXT));
  assert_eq!(s.vault.stores.load(Ordering::SeqCst), 2);
  assert!(!s.window.link.input_gate().protecting());
}
