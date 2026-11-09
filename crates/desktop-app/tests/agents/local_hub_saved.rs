//! What a local MCP server's panel shows once a save went through (ORC-848 D9): the value that was saved, with nothing
//! left unsaved and no question about what it widens. `cargo test -p orchester-app --test agents local_hub_saved`.

use orchester_core::store::McpServerStore;
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  local_hub::{SAVE, server, show},
};

fn exempt(agents: &Agents) -> Vec<String> {
  let db = ProjectDb::open(&agents.root).expect("database");
  McpServerStore::mcp_servers(&db)
    .expect("servers")
    .iter()
    .flat_map(|server| {
      server
        .exempt_tools()
        .tools()
        .iter()
        .map(|tool| tool.as_str().to_owned())
        .collect::<Vec<_>>()
    })
    .collect()
}

#[test]
fn local_hub_saved_the_tools_that_run_without_asking_stay_as_typed_and_nothing_is_left_to_save() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  show(&mut agents, docs);
  agents.set("connection-mcp-tools", "search_docs");
  agents.until("saveable", |s| s.exists(SAVE));
  agents.click(SAVE);
  agents.until("the question", |s| s.exists("local-hub-mcp-confirm-save-confirm"));
  agents.press_until("local-hub-mcp-confirm-save-confirm", "saved", |s| {
    exempt(s) == ["search_docs"]
  });

  // The panel opens again on what was saved once the project's connections are read again: the value shows with nothing
  // left to save, which the panel it replaced (the value typed, Save offered) and the one opened on the old text (no
  // value) are not.
  agents.until("the value saved shows", |s| {
    !s.exists(SAVE) && s.value("connection-mcp-tools") == "search_docs"
  });
  for _ in 0..20 {
    agents.until("a pass", |_| true);
  }
  assert!(!agents.exists(SAVE), "no save is offered again");
  assert!(!agents.exists("local-hub-mcp-confirm-save"), "no widening is flagged");
  assert_eq!(agents.value("connection-mcp-tools"), "search_docs");
}
