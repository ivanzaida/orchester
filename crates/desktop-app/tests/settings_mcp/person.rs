//! Only a person changes the server's settings: input lurq delivers as synthetic (an MCP client's) is not taken,
//! and the row says so (`eM8tB`, *Input from automation not taken*).

use lurq::app::events::with_synthetic_input;

use super::*;

#[test]
fn settings_mcp_synthetic_input_changes_nothing() {
  let port = free_port();
  let mut shown = Shown::installed(port, Arc::default());
  shown.until("listening", |shown| shown.says("settings-mcp-state", "Listening"));
  let token = shown.token();
  let gate = shown.link.input_gate();
  for (control, blocked) in [
    ("settings-mcp-enabled-switch", "settings-mcp-enabled-blocked"),
    ("settings-mcp-token-show", "settings-mcp-token-blocked"),
    ("settings-mcp-token-rotate", "settings-mcp-token-blocked"),
  ] {
    with_synthetic_input(|| shown.click(control));
    assert!(
      shown.texts(blocked).is_some(),
      "{control}: the row says it was not taken"
    );
    assert!(shown.says(
      blocked,
      "A change brought by automation was not taken: only you change the MCP server's settings, here in the window."
    ));
  }
  shown.tree.tick_futures();
  shown.pass();
  assert!(shown.says("settings-mcp-state", "Listening"), "still on");
  assert!(!gate.token_shown(), "not shown");
  assert!(shown.texts("settings-mcp-token-rotate-question").is_none(), "not asked");
  assert_eq!(shown.token(), token, "not rotated");
  shown
    .tree
    .get_element_by_id_mut("settings-mcp-port")
    .and_then(|element| element.as_text_input())
    .expect("the port field is a text input")
    .set_value(free_port().to_string());
  shown.pass();
  with_synthetic_input(|| shown.click("settings-mcp-use-port"));
  assert!(shown.texts("settings-mcp-address-blocked").is_some());
  shown.tree.tick_futures();
  shown.pass();
  assert_eq!(
    shown.server.port().map(std::num::NonZeroU16::get),
    Some(port),
    "not moved"
  );
}
