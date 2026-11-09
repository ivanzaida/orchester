//! What a refused press says in the local hub: input an MCP client sent where only a person answers is said so in the
//! panel, never dropped without a word (ORC-848 D1, D2). `cargo test -p orchester-app --test agents local_hub_refus`.

use super::{
  harness::Agents,
  local_hub::{SAVE, server, show},
};

#[test]
fn local_hub_refusals_a_clients_press_on_the_save_question_is_said_to_need_a_person() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  show(&mut agents, docs);
  agents.set("connection-mcp-arguments", "--stdio");
  agents.until("saveable", |s| s.exists(SAVE));
  agents.click(SAVE);
  agents.until("the question", |s| s.exists("local-hub-mcp-confirm-save-confirm"));
  assert!(
    !agents.exists("local-hub-mcp-confirm-save-person-only"),
    "nothing was refused yet"
  );

  // A press that reaches the confirm's handler as an MCP client's input; the confirm takes none before it lived
  // through a pass, so it is pressed until it answers.
  agents.until("the refusal", |s| {
    lurq::app::events::with_synthetic_input(|| s.click("local-hub-mcp-confirm-save-confirm"));
    s.exists("local-hub-mcp-confirm-save-person-only")
  });
  assert!(
    agents
      .text("local-hub-mcp-confirm-save-person-only")
      .starts_with("Only a person does this here")
  );
  assert!(
    agents.exists("local-hub-mcp-confirm-save-confirm"),
    "the question stays open"
  );
}

#[test]
fn local_hub_refusals_input_the_gate_refuses_while_the_save_question_shows_is_said_in_the_window() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  show(&mut agents, docs);
  agents.set("connection-mcp-arguments", "--stdio");
  agents.until("saveable", |s| s.exists(SAVE));
  agents.click(SAVE);
  agents.until("the question", |s| s.exists("local-hub-mcp-confirm-save-confirm"));
  assert!(!agents.exists("local-hub-mcp-confirm-save-person-only"));

  let said = agents.window.link.input_gate().refuse_input();
  assert!(said.contains("A person answers this in the window"), "{said}");
  agents.until("the refusal", |s| s.exists("local-hub-mcp-confirm-save-person-only"));
}
