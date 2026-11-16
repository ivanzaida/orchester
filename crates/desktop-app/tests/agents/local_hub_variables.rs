//! A local MCP server's *Variables* as the panel shows them (L2 `IpzZ7`, L12): one that takes a secret reads `NAME ←
//! secret SECRET`, while what is stored stays `NAME=${secret:SECRET@environment}`. `cargo test -p orchester-app --test
//! agents local_hub_variables`.

use orchester_core::{
  connection::mcp::McpEnvValue,
  store::{McpServerStore, SecretStore},
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  local_hub::{SAVE, figma, show},
};

const VARIABLES: &str = "connection-mcp-variables";

#[test]
fn local_hub_variables_a_secret_reads_as_an_arrow_and_is_stored_as_its_reference() {
  let mut agents = Agents::new(true);
  let figma = figma(&agents);
  show(&mut agents, figma);
  assert_eq!(agents.value(VARIABLES), "FIGMA_API_KEY ← secret FIGMA_TOKEN");

  // A variable typed beside it is a change; the one that takes the secret keeps its reference when saved.
  agents.set(VARIABLES, "FIGMA_API_KEY ← secret FIGMA_TOKEN\nDEBUG=1");
  agents.until("saveable", |s| s.exists("local-hub-mcp-unsaved"));
  agents.click(SAVE);
  let db = ProjectDb::open(&agents.root).expect("database");
  let token = db.secrets().expect("secrets").remove(0).id();
  let saved = move |s: &mut Agents| {
    let db = ProjectDb::open(&s.root).expect("database");
    McpServerStore::mcp_servers(&db)
      .expect("servers")
      .iter()
      .find(|server| server.id() == figma)
      .is_some_and(|server| {
        let variables: Vec<_> = server
          .environment()
          .iter()
          .map(|(name, value)| (name.as_str().to_owned(), value.clone()))
          .collect();
        variables
          == [
            ("DEBUG".to_owned(), McpEnvValue::Text("1".to_owned())),
            ("FIGMA_API_KEY".to_owned(), McpEnvValue::Secret(token)),
          ]
      })
  };
  // A variable more widens what agents may do: the panel asks first.
  let confirm = "local-hub-mcp-confirm-save-confirm";
  agents.until("the question", |s| {
    s.exists(confirm) || s.exists("local-hub-mcp-refused")
  });
  assert!(
    !agents.exists("local-hub-mcp-refused"),
    "{}",
    agents.text("local-hub-mcp-refused")
  );
  agents.press_until(confirm, "saved", saved);
  agents.until("shown again as saved", |s| {
    !s.exists("local-hub-mcp-unsaved") && s.value(VARIABLES) == "DEBUG=1\nFIGMA_API_KEY ← secret FIGMA_TOKEN"
  });
}
