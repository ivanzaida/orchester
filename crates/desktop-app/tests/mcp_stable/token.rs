//! The token: stored once per installation, replaced in place, kept when the store refuses the new one, said when it
//! cannot be read, and hidden from clients while it shows in the window.

use super::*;

#[test]
fn app_mcp_token_is_stored_once_per_installation() {
  let installed = Installed::start(free_port());
  let values = installed.vault.values();
  assert_eq!(values.len(), 1, "one token");
  let (key, stored) = values.into_iter().next().expect("the token");
  assert_eq!(key.service(), "orchester-app-mcp");
  assert_eq!(
    key.account(),
    installed.workspace.installation_id().expect("read").to_string()
  );
  assert_eq!(stored, installed.token());
  assert_eq!(stored.len(), 64);
  let other = Installed::start(free_port());
  assert_ne!(other.token(), installed.token(), "another installation has its own");
}

#[test]
fn app_mcp_rotate_replaces_the_token_and_ends_the_old_sessions() {
  let installed = Installed::start(free_port());
  let (port, old) = (installed.port(), installed.token());
  let session = initialize(port, &old, ("claude-code", "2.1.296"));
  installed.server.rotate_token().expect("rotated");
  let status = installed.server.status();
  assert!(status.rotated && status.clients.is_empty(), "the list starts again");
  let new = installed.token();
  assert_ne!(new, old);
  assert_eq!(knocks(port, &old), 401);
  assert_eq!(knocks(port, &new), 200);
  let list = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} });
  let gone = post(
    port,
    &[("Authorization", &bearer(&new)), ("Mcp-Session-Id", &session)],
    &list,
  );
  assert_ne!(gone.status, 200, "the old session ended: {}", gone.text);
  assert!(installed.vault.values().values().any(|stored| *stored == new));
  assert_eq!(installed.port(), port, "the port stays");
}

#[test]
fn app_mcp_rotate_refused_by_the_store_keeps_the_old_token() {
  let installed = Installed::start(free_port());
  let (port, old) = (installed.port(), installed.token());
  installed.vault.refuse_stores(true);
  assert!(matches!(
    installed.server.rotate_token(),
    Err(ControlError::Vault(VaultError::AccessDenied(_)))
  ));
  assert_eq!(installed.token(), old);
  assert_eq!(knocks(port, &old), 200);
  assert!(!installed.server.status().rotated);
}

#[test]
fn app_mcp_unreadable_token_keeps_the_server_off_until_it_can_be_read() {
  let data = Data::new();
  let vault = Arc::new(MemoryVault::default());
  let port = free_port();
  let installed = Installed::start_with(data, vault.clone(), Some(port));
  let token = installed.token();
  let installed = {
    vault.refuse_reads(true);
    installed.restart()
  };
  let ServerState::TokenUnreadable { reason } = installed.server.status().server else {
    panic!("the token cannot be read: {:?}", installed.server.status().server);
  };
  assert_eq!(
    reason,
    orchester_workspace::messages::secret_store_error(&VaultError::AccessDenied(String::new()))
  );
  assert_eq!(installed.server.port(), None);
  vault.refuse_reads(false);
  installed.server.try_again();
  assert_eq!(installed.port(), port);
  assert_eq!(installed.token(), token, "the stored token, not a new one");
}

#[test]
fn app_mcp_refuses_screenshots_and_window_text_while_the_token_shows() {
  let installed = Installed::start(free_port());
  let (port, token) = (installed.port(), installed.token());
  let session = initialize(port, &token, ("claude-code", "2.1.296"));
  let gate = installed.link.input_gate();
  let shown = gate.show_token();
  for tool in ["lurq_screenshot", "lurq_read_tree", "lurq_find", "lurq_inspect"] {
    let response = call(port, &token, &session, tool);
    let result = &response.message.as_ref().expect("the call answers")["result"];
    assert_eq!(result["isError"], true, "{tool}: {}", response.text);
    let refusal: Value =
      serde_json::from_str(result["content"][0]["text"].as_str().expect("a refusal")).expect("the refusal is JSON");
    assert_eq!(refusal["code"], "token_shown", "{tool}");
    assert!(!response.text.contains(&token));
  }
  drop(shown);
  assert!(!gate.token_shown());
}
