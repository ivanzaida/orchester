//! A refused press in the local hub: input an MCP client sent where only a person answers is refused to the client,
//! as its tool call's error, and the panel stays as it was (the person, 2026-11-10). `cargo test -p orchester-app
//! --test agents local_hub_refus`.

use super::{
  harness::Agents,
  local_hub::{SAVE, server, show},
};

#[test]
fn local_hub_refusals_a_clients_press_on_the_save_question_is_refused_to_it_and_changes_nothing() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  show(&mut agents, docs);
  agents.set("connection-mcp-arguments", "--stdio");
  agents.until("saveable", |s| s.exists(SAVE));
  agents.click(SAVE);
  agents.until("the question", |s| s.exists("local-hub-mcp-confirm-save-confirm"));
  assert_eq!(agents.refused(), None, "nothing was refused yet");

  // A press that reaches the confirm's handler as an MCP client's input; the confirm takes none before it lived
  // through a pass, so it is pressed until it is refused.
  agents.until("the refusal", |s| {
    lurq::app::events::with_synthetic_input(|| s.click("local-hub-mcp-confirm-save-confirm"));
    s.refused().is_some()
  });
  assert!(
    agents.exists("local-hub-mcp-confirm-save-confirm"),
    "the question stays open"
  );
  assert!(
    !agents.exists("local-hub-mcp-confirm-save-person-only"),
    "nothing in the window says it"
  );
  assert_eq!(
    agents.value("connection-mcp-arguments"),
    "--stdio",
    "the field keeps what was typed"
  );

  // The person's own press answers it.
  agents.press_until("local-hub-mcp-confirm-save-confirm", "the person's save", |s| {
    !s.exists("local-hub-mcp-confirm-save-confirm")
  });
  assert_eq!(agents.refused(), None);
}

#[test]
fn local_hub_refusals_input_the_gate_refuses_while_the_save_question_shows_leaves_the_window_as_it_was() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  show(&mut agents, docs);
  agents.set("connection-mcp-arguments", "--stdio");
  agents.until("saveable", |s| s.exists(SAVE));
  agents.click(SAVE);
  agents.until("the question", |s| s.exists("local-hub-mcp-confirm-save-confirm"));

  let gate = agents.window.link.input_gate();
  assert!(gate.protecting(), "the question locks window input");
  let said = gate.refusal();
  assert!(said.contains("A person answers this in the window"), "{said}");
  agents.window.pass();
  assert!(!agents.exists("local-hub-mcp-confirm-save-person-only"));
  assert!(agents.exists("local-hub-mcp-confirm-save-confirm"));
}
