//! The server's states as the section and the status bar say them (`lye6f`, `eTrCO`, `QqBCj`).

use super::*;

#[test]
fn settings_mcp_listening_names_the_address_and_never_the_token() {
  let port = free_port();
  let mut shown = Shown::installed(port, Arc::default());
  shown.until("listening", Shown::listening);
  for words in [
    "Orchester MCP server",
    "This installation",
    "Let agents connect to Orchester",
    "Address",
    &format!("127.0.0.1:{port}"),
    "Access token",
    "•••• •••• •••• •••• •••• •••• •••• ••••",
    "Connect an agent",
    "Agents connected",
    "No agent connected since Orchester started.",
  ] {
    assert!(
      shown.says("settings-mcp", words),
      "{words} in {:?}",
      shown.texts("settings-mcp")
    );
  }
  let token = shown.token();
  assert!(
    !shown.everything().iter().any(|text| text.contains(&token)),
    "the token never shows unasked"
  );
  assert!(shown.says("status-mcp", &format!("MCP 127.0.0.1:{port}")));
  assert!(
    shown.link.input_gate().protecting(),
    "while the section shows, clients' window input is refused"
  );
}

#[test]
fn settings_mcp_port_taken_is_said_and_another_port_moves_the_server() {
  let holder = TcpListener::bind(("127.0.0.1", 0)).expect("another program's port");
  let port = holder.local_addr().expect("its address").port();
  let mut shown = Shown::installed(port, Arc::default());
  let taken = format!("Port {port} taken");
  shown.until("port taken", |shown| shown.says("settings-mcp-state", &taken));
  assert!(
    shown.texts("settings-mcp-port-taken").is_some(),
    "the line under the field"
  );
  assert!(shown.says("status-mcp", &format!("MCP port {port} taken")));
  let next = free_port();
  type_port(&mut shown, &next.to_string());
  shown.click("settings-mcp-use-port");
  shown.until("listening on the new port", |shown| {
    shown.says("status-mcp", &format!("MCP 127.0.0.1:{next}"))
  });
  assert!(shown.says("settings-mcp", &format!("127.0.0.1:{next}")));
  drop(holder);
}

#[test]
fn settings_mcp_port_outside_the_range_is_not_taken() {
  let mut shown = Shown::installed(free_port(), Arc::default());
  shown.until("listening", Shown::listening);
  // *Port* opens the field under the address.
  shown.click("settings-mcp-port-edit");
  type_port(&mut shown, "80");
  shown.click("settings-mcp-use-port");
  shown.until("the field's error", |shown| {
    shown.texts("settings-mcp-port-invalid").is_some()
  });
  assert!(shown.says("settings-mcp-port-invalid", "Use a port from 1024 to 65535."));
}

#[test]
fn settings_mcp_off_listens_nowhere_until_turned_on() {
  let port = free_port();
  let mut shown = Shown::installed(port, Arc::default());
  shown.until("listening", Shown::listening);
  shown.click("settings-mcp-enabled-switch");
  shown.until("off", |shown| shown.says("settings-mcp-state", "Off"));
  assert!(shown.says("status-mcp", "MCP off"));
  assert!(shown.says(
    "settings-mcp",
    "Nothing listens while the server is off. The address and the token stay as they are."
  ));
  shown.click("settings-mcp-enabled-switch");
  shown.until("listening again", |shown| {
    shown.says("status-mcp", &format!("MCP 127.0.0.1:{port}"))
  });
}

#[test]
fn settings_mcp_unreadable_token_says_why_and_tries_again() {
  let vault = Arc::new(MemoryVault::default());
  vault.refuse_reads(true);
  let mut shown = Shown::installed(free_port(), vault.clone());
  shown.until("not running", |shown| shown.says("settings-mcp-state", "Not running"));
  assert!(shown.says(
    "settings-mcp-unreadable",
    &format!("The token can't be read from {STORE}")
  ));
  assert!(shown.says("status-mcp", "MCP unavailable"));
  let body = shown
    .texts("settings-mcp-unreadable")
    .expect("the notice")
    .into_iter()
    .find(|text| text.ends_with("rotate the token to store a new one."))
    .expect("its body");
  assert!(body.starts_with(DENIED), "the store's own short words: {body}");
  assert!(shown.says("settings-mcp", "No address while the server is not running."));
  vault.refuse_reads(false);
  shown.click("settings-mcp-unreadable-again");
  shown.until("listening", Shown::listening);
  assert!(shown.texts("settings-mcp-unreadable").is_none());
}

#[test]
fn settings_mcp_qa_copy_says_where_its_discovery_file_is() {
  let mut shown = Shown::qa();
  let port = shown.server.port().expect("the copy listens");
  shown.until("the copy's line", |shown| {
    shown.texts("settings-mcp-discovery").is_some()
  });
  for words in ["This QA copy", "Server", &format!("Listening on 127.0.0.1:{port}")] {
    assert!(
      shown.says("settings-mcp", words),
      "{words} in {:?}",
      shown.texts("settings-mcp")
    );
  }
  let folder = std::path::PathBuf::from(".tmp").join("qa").join("task").join("mcp");
  let note = shown.joined("settings-mcp-discovery");
  assert!(
    note.contains(&format!("its discovery file is in {}.", folder.display())),
    "the folder from .tmp on (eTrCO): {note}"
  );
  assert!(shown.texts("settings-mcp-token-row").is_none(), "no token row");
  assert!(shown.texts("settings-mcp-enabled-switch").is_none(), "no switch");
  assert!(shown.says("status-mcp", &format!("MCP :{port} · QA")));
}

/// Puts `port` in the port field.
fn type_port(shown: &mut Shown, port: &str) {
  shown
    .tree
    .get_element_by_id_mut("settings-mcp-port")
    .and_then(|element| element.as_text_input())
    .expect("the port field is a text input")
    .set_value(port);
  shown.pass();
}

#[test]
fn settings_mcp_unreadable_token_offers_no_commands() {
  let vault = Arc::new(MemoryVault::default());
  vault.refuse_reads(true);
  let mut shown = Shown::installed(free_port(), vault);
  shown.until("not running", |shown| shown.says("settings-mcp-state", "Not running"));
  assert!(shown.texts("settings-mcp-connect-claude-command").is_none());
  assert!(shown.says(
    "settings-mcp-connect-unavailable",
    "The commands need the access token, which can't be read now."
  ));
}

#[test]
fn settings_mcp_status_popover_opens_over_the_bar_at_the_window_end() {
  let mut shown = Shown::installed(free_port(), Arc::default());
  shown.until("listening", Shown::listening);
  shown.click("status-mcp");
  let [x, y, width, height] = shown.bounds("status-mcp-popover");
  let [bar_x, bar_y, bar_width, _] = shown.bounds("status-bar");
  assert_eq!(bar_y - (y + height), 9.0, "9 above the bar (QqBCj)");
  assert_eq!(
    (bar_x + bar_width) - (x + width),
    12.0,
    "its end 12 from the window's (QqBCj)"
  );
}

#[test]
fn settings_mcp_address_copy_is_unavailable_while_nothing_listens() {
  let holder = TcpListener::bind(("127.0.0.1", 0)).expect("another program's port");
  let port = holder.local_addr().expect("its address").port();
  let mut shown = Shown::installed(port, Arc::default());
  shown.until("port taken", |shown| shown.texts("settings-mcp-port-taken").is_some());
  // The address shows with its Copy, which takes no press while nothing listens there (eTrCO).
  assert!(shown.says("settings-mcp-address-value", &format!("127.0.0.1:{port}")));
  shown.click("settings-mcp-address-value-copy");
  assert!(
    !shown.everything().iter().any(|text| text.starts_with("Copied")),
    "unavailable: nothing was copied"
  );
  drop(holder);
}

#[test]
fn settings_mcp_address_copy_says_what_came_of_it() {
  let mut shown = Shown::installed(free_port(), Arc::default());
  shown.until("listening", Shown::listening);
  assert!(shown.texts("settings-mcp-address-value-copied").is_none());
  shown.click("settings-mcp-address-value-copy");
  // Copy says it for a moment, as the command row's Copy did: copied, or that the clipboard refused it.
  assert!(
    shown.says("settings-mcp-address-value-copied", "Copied")
      || shown.says("settings-mcp-address-value-copied", "Couldn't copy"),
    "Copy says what came of it"
  );
}
