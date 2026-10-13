//! The access token's row (`eM8tB`): rotated after a confirmation in place, kept when the store refuses the new one,
//! and shown on a person's click, while clients may not read the window, until it hides again.

use super::*;

#[test]
fn settings_mcp_rotate_asks_in_place_then_uses_a_new_token() {
  let vault = Arc::new(MemoryVault::default());
  let mut shown = Shown::installed(free_port(), vault.clone());
  shown.until("listening", |shown| shown.says("settings-mcp-state", "Listening"));
  let old = shown.token();
  shown.click("settings-mcp-token-rotate");
  assert!(shown.says("settings-mcp-token-rotate-question", "Rotate the token?"));
  shown.click("settings-mcp-token-rotate-keep");
  assert!(shown.texts("settings-mcp-token-rotate-question").is_none());
  assert_eq!(shown.token(), old, "kept");
  shown.click("settings-mcp-token-rotate");
  shown.click("settings-mcp-token-rotate-confirm");
  shown.until("the new token's line", |shown| {
    shown.texts("settings-mcp-token-rotated").is_some()
  });
  let new = shown.token();
  assert_ne!(new, old);
  assert!(vault.values().values().any(|stored| *stored == new), "stored first");
  assert!(shown.says("settings-mcp", "No agent connected since the token changed."));
  assert!(!shown.everything().iter().any(|text| text.contains(&new)));
}

#[test]
fn settings_mcp_rotate_refused_by_the_store_keeps_the_token_and_says_so() {
  let vault = Arc::new(MemoryVault::default());
  let mut shown = Shown::installed(free_port(), vault.clone());
  shown.until("listening", |shown| shown.says("settings-mcp-state", "Listening"));
  let old = shown.token();
  vault.refuse_stores(true);
  shown.click("settings-mcp-token-rotate");
  shown.click("settings-mcp-token-rotate-confirm");
  shown.until("the refusal", |shown| {
    shown.texts("settings-mcp-token-not-rotated").is_some()
  });
  assert_eq!(shown.token(), old);
  assert!(shown.texts("settings-mcp-token-rotate-again").is_some(), "Try again");
}

#[test]
fn settings_mcp_show_hides_the_window_from_clients_until_hidden() {
  let mut shown = Shown::installed(free_port(), Arc::default());
  shown.until("listening", |shown| shown.says("settings-mcp-state", "Listening"));
  let gate = shown.link.input_gate();
  shown.click("settings-mcp-token-show");
  assert!(gate.token_shown(), "clients stop reading the window");
  assert!(shown.says("settings-mcp-token-show", "Hide"));
  assert!(shown.says(
    "settings-mcp-token-shown-for",
    "Hidden again in 30 s. While it shows, the MCP server refuses screenshots and the window's text, so no agent \
     reads it."
  ));
  shown.click("settings-mcp-token-show");
  assert!(!gate.token_shown(), "hidden, clients read the window again");
  assert!(shown.says("settings-mcp-token-value", "•••• •••• •••• •••• •••• •••• •••• ••••"));
}

#[test]
fn settings_mcp_leaving_settings_hides_the_token() {
  let mut shown = Shown::installed(free_port(), Arc::default());
  shown.until("listening", |shown| shown.says("settings-mcp-state", "Listening"));
  let gate = shown.link.input_gate();
  shown.click("settings-mcp-token-show");
  assert!(gate.token_shown());
  shown
    .link
    .navigation()
    .expect("the window's navigation")
    .go(Route::AllProjects);
  shown.until("Settings closed", |shown| shown.texts("settings-mcp").is_none());
  assert!(!gate.token_shown(), "the token went with the page");
  assert!(!gate.protecting(), "and so did the person-only controls");
}

#[test]
fn settings_mcp_token_field_and_buttons_keep_their_place_when_shown() {
  const PARTS: [&str; 4] = [
    "settings-mcp-token-value",
    "settings-mcp-token-show",
    "settings-mcp-token-copy",
    "settings-mcp-token-rotate",
  ];
  let mut shown = Shown::installed(free_port(), Arc::default());
  shown.until("listening", |shown| shown.says("settings-mcp-state", "Listening"));
  let hidden = PARTS.map(|id| shown.bounds(id));
  shown.click("settings-mcp-token-show");
  assert!(shown.link.input_gate().token_shown());
  let revealed = PARTS.map(|id| shown.bounds(id));
  for ((id, hidden), revealed) in PARTS.iter().zip(hidden).zip(revealed) {
    assert_eq!(hidden, revealed, "{id} moved or changed width when the token showed");
  }
  let field = revealed[0];
  assert!(
    field[2] > 400.0,
    "the field takes the room the buttons leave (lye6f: about 430): {field:?}"
  );
  let text = shown.bounds("settings-mcp-token-text");
  assert!(
    text[0] + text[2] <= field[0] + field[2],
    "the token stays inside its field: {text:?} in {field:?}"
  );
}
