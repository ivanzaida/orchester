//! Answering in place with an MCP client's input (lurq's synthetic input): while automation may not answer, the
//! card refuses it and says that only a person answers here, and the person's own press still answers; once the
//! person lets automation answer approvals and change agent permissions, the client's Approve once goes to the
//! runner as automation's; a wider answer stays a person's. Recorded as whose is `input_gate::answerer`'s rule
//! (`tests/automation_permissions.rs`); here no runner takes the answer, so the card says why it did not go.

use lurq::app::events::with_synthetic_input;

use super::{
  Window, click, expand_approval,
  in_place::{expand_question, press},
  open, seeded, settle,
};

const PERSON_ONLY: &str = "Only a person answers this here: input from an MCP client answers only while you let \
                           automation answer approvals and change agent permissions.";
const WIDER: &str = "Only a person gives a wider answer: automation approves a request once or denies it.";

fn refusal(window: &mut Window, card: &str) -> Option<String> {
  window.texts(&format!("{card}-refused")).map(|texts| texts.join(" "))
}

/// Lets automation answer approvals and change agent permissions, as the person does in Settings.
fn consent(window: &Window) {
  let workspace = window.launch.workspace().expect("the session started");
  workspace
    .set_automation_answers_approvals(true)
    .expect("the approvals opt-in");
  workspace
    .set_automation_changes_permissions(true)
    .expect("the permissions opt-in");
}

#[test]
fn needs_you_panel_refuses_an_mcp_clients_approval_without_consent() {
  let (mut window, _) = seeded("cards");
  open(&mut window);
  let (_, card) = expand_approval(&mut window, "Add serde to the webhook payloads");
  let approve = format!("{card}-approve");
  window.until("Approve once", |window| {
    window.tree.get_element_by_id(&approve).is_some()
  });
  with_synthetic_input(|| click(&mut window, &approve));
  window.until("why the client's answer was not taken", |window| {
    refusal(window, &card).is_some()
  });
  assert_eq!(refusal(&mut window, &card).as_deref(), Some(PERSON_ONLY));
  with_synthetic_input(|| click(&mut window, &format!("{card}-deny")));
  settle(&mut window);
  assert_eq!(refusal(&mut window, &card).as_deref(), Some(PERSON_ONLY), "Deny too");
  // The person's own press answers: it goes to the runner, none here, and the card says that instead.
  click(&mut window, &approve);
  window.until("the person's answer sent", |window| {
    refusal(window, &card).is_some_and(|refused| refused != PERSON_ONLY)
  });
}

#[test]
fn needs_you_panel_sends_an_mcp_clients_approval_as_automation_under_consent() {
  let (mut window, _) = seeded("cards");
  consent(&window);
  open(&mut window);
  let (_, card) = expand_approval(&mut window, "Add serde to the webhook payloads");
  let approve = format!("{card}-approve");
  window.until("Approve once", |window| {
    window.tree.get_element_by_id(&approve).is_some()
  });
  with_synthetic_input(|| click(&mut window, &approve));
  window.until("the client's answer sent", |window| {
    refusal(window, &card).is_some_and(|refused| refused != PERSON_ONLY)
  });
  // A wider answer stays a person's even under consent: its confirm takes the client's input, the card refuses it.
  let similar = format!("{card}-similar");
  click(&mut window, &similar);
  let confirm = format!("{card}-widen-confirm");
  window.until("the question in place", |window| {
    window.tree.get_element_by_id(&confirm).is_some()
  });
  settle(&mut window);
  with_synthetic_input(|| click(&mut window, &confirm));
  window.until("why the wider answer was not taken", |window| {
    refusal(window, &card).as_deref() == Some(WIDER)
  });
}

#[test]
fn needs_you_panel_refuses_an_mcp_clients_answer_to_a_question_without_consent() {
  let (mut window, _) = seeded("cards");
  open(&mut window);
  let (_, card) = expand_question(&mut window, "Pick where the docs button goes");
  let option = format!("{card}-option-0-1");
  window.until("the options", |window| window.tree.get_element_by_id(&option).is_some());
  click(&mut window, &option);
  with_synthetic_input(|| click(&mut window, &format!("{card}-send")));
  window.until("why the client's answer was not taken", |window| {
    refusal(window, &card).is_some()
  });
  assert_eq!(refusal(&mut window, &card).as_deref(), Some(PERSON_ONLY));
  // Enter in the own answer is the same press.
  let own = format!("{card}-own-0");
  click(&mut window, &own);
  press(&mut window, "x", "KeyX");
  with_synthetic_input(|| press(&mut window, "Enter", "Enter"));
  settle(&mut window);
  assert_eq!(refusal(&mut window, &card).as_deref(), Some(PERSON_ONLY), "Enter too");
  press(&mut window, "Enter", "Enter");
  window.until("the person's answer sent", |window| {
    refusal(window, &card).is_some_and(|refused| refused != PERSON_ONLY)
  });
}
