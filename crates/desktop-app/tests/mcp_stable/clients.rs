//! *Agents connected*: the clients as they named themselves, connected while a session of theirs is open.

use super::*;

#[test]
fn app_mcp_lists_clients_as_they_name_themselves_while_connected() {
  let installed = Installed::start(free_port());
  let (port, token) = (installed.port(), installed.token());
  assert!(installed.server.status().clients.is_empty());
  let claude = initialize(port, &token, ("claude-code", "2.1.296"));
  let codex = initialize(port, &token, ("codex-mcp-client", "0.159.2"));
  let again = initialize(port, &token, ("claude-code", "2.1.296"));
  let clients = installed.server.status().clients;
  let named: Vec<(&str, &str, bool)> = clients
    .iter()
    .map(|client| (client.name.as_str(), client.version.as_str(), client.connected))
    .collect();
  assert_eq!(
    named,
    [("claude-code", "2.1.296", true), ("codex-mcp-client", "0.159.2", true)],
    "one entry per client and version"
  );
  let auth = bearer(&token);
  for session in [&claude, &again] {
    let ended = delete(port, &[("Authorization", &auth), ("Mcp-Session-Id", session)]);
    assert!(ended.status < 300, "{}", ended.text);
  }
  let connected = wait_until(|| {
    let clients = installed.server.status().clients;
    clients
      .iter()
      .any(|client| client.name == "claude-code" && !client.connected)
  });
  assert!(connected, "claude-code is not connected once both its sessions ended");
  assert!(
    installed
      .server
      .status()
      .clients
      .iter()
      .any(|client| client.name == "codex-mcp-client" && client.connected)
  );
  drop(codex);
}

/// Whether `done` holds within a few seconds: a session ends on the server's runtime.
fn wait_until(done: impl Fn() -> bool) -> bool {
  let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
  while std::time::Instant::now() < until {
    if done() {
      return true;
    }
    std::thread::sleep(std::time::Duration::from_millis(20));
  }
  done()
}
