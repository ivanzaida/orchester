//! Only a person changes the server's settings: while they show, the input gate refuses an MCP client's window input
//! (`Guarded::McpServer`), and synthetic input that reaches a control all the same is not taken and only logged: the
//! row stays as it was.

use lurq::app::events::with_synthetic_input;

use super::*;

#[test]
fn settings_mcp_synthetic_input_changes_nothing() {
  let port = free_port();
  let mut shown = Shown::installed(port, Arc::default());
  shown.until("listening", Shown::listening);
  let token = shown.token();
  let gate = shown.link.input_gate();
  assert!(gate.protecting(), "the gate refuses an MCP client's window input here");
  for (control, blocked) in [
    ("settings-mcp-enabled-switch", "settings-mcp-enabled-blocked"),
    ("settings-mcp-token-show", "settings-mcp-token-blocked"),
    ("settings-mcp-token-rotate", "settings-mcp-token-blocked"),
  ] {
    with_synthetic_input(|| shown.click(control));
    assert!(shown.texts(blocked).is_none(), "{control}: the row says nothing of it");
  }
  shown.tree.tick_futures();
  shown.pass();
  assert!(shown.listening(), "still on");
  assert!(!gate.token_shown(), "not shown");
  assert!(shown.texts("settings-mcp-token-rotate-question").is_none(), "not asked");
  assert_eq!(shown.token(), token, "not rotated");
  // *Port* opens the field under the address.
  shown.click("settings-mcp-port-edit");
  shown
    .tree
    .get_element_by_id_mut("settings-mcp-port")
    .and_then(|element| element.as_text_input())
    .expect("the port field is a text input")
    .set_value(free_port().to_string());
  shown.pass();
  with_synthetic_input(|| shown.click("settings-mcp-use-port"));
  assert!(shown.texts("settings-mcp-address-blocked").is_none());
  shown.tree.tick_futures();
  shown.pass();
  assert_eq!(
    shown.server.port().map(std::num::NonZeroU16::get),
    Some(port),
    "not moved"
  );
}

/// The route that asks *Rotate the token?* opens the question in place (`shpsk`), as a QA copy's `navigate` does; only
/// a person's *Rotate token* answers it.
#[test]
fn settings_mcp_the_route_asks_to_rotate_and_only_a_person_answers() {
  let mut shown = Shown::installed(free_port(), Arc::default());
  shown.until("listening", Shown::listening);
  let token = shown.token();
  shown
    .link
    .navigation()
    .expect("the window's navigation")
    .go(Route::RotateToken);
  shown.until("the question", |shown| {
    shown.says("settings-mcp-token-rotate-question", "Rotate the token?")
  });
  with_synthetic_input(|| shown.click("settings-mcp-token-rotate-confirm"));
  assert!(shown.texts("settings-mcp-token-blocked").is_none(), "nothing said");
  assert!(
    shown.texts("settings-mcp-token-rotate-question").is_some(),
    "still asked"
  );
  shown.tree.tick_futures();
  shown.pass();
  assert_eq!(shown.token(), token, "not rotated");
  shown.click("settings-mcp-token-rotate-keep");
  assert!(shown.texts("settings-mcp-token-rotate-question").is_none(), "kept");
}
