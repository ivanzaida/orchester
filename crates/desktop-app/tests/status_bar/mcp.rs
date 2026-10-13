//! The MCP item in each state of the app's server (`QqBCj`, `eTrCO`) and its popover: the address or why there is
//! none, the agents connected, whether agents may control the window, and *Open Automation settings*.

use orchester_app::mcp::SeenClient;
use orchester_core::app_mcp::McpPort;

use super::*;

fn with(server: ServerState, kind: McpKind) -> BarFacts {
  BarFacts {
    mcp: McpStatus {
      server,
      kind,
      ..listening()
    },
    ..facts(None)
  }
}

fn port(port: u16) -> NonZeroU16 {
  NonZeroU16::new(port).expect("a port")
}

fn client(name: &str, version: &str, connected: bool) -> SeenClient {
  SeenClient {
    name: name.to_owned(),
    version: version.to_owned(),
    connected,
    last_request: SystemTime::now(),
  }
}

#[test]
fn status_mcp_item_names_the_address_or_why_agents_cannot_reach_it() {
  let taken = ServerState::PortTaken { port: McpPort::DEFAULT };
  let unreadable = ServerState::TokenUnreadable {
    reason: Message::new("errors.keychainDenied"),
  };
  let qa = McpKind::Qa {
    folder: PathBuf::from(".tmp/qa/orc-31/mcp"),
    file: None,
  };
  let development = McpKind::Development { file: None };
  for (server, kind, words) in [
    (
      ServerState::Listening { port: port(47821) },
      McpKind::Installation,
      "MCP 127.0.0.1:47821",
    ),
    (taken, McpKind::Installation, "MCP port 47821 taken"),
    (ServerState::Off, McpKind::Installation, "MCP off"),
    (unreadable, McpKind::Installation, "MCP unavailable"),
    (ServerState::Unavailable, McpKind::Installation, "MCP unavailable"),
    (ServerState::Listening { port: port(53122) }, qa, "MCP :53122 · QA"),
    (ServerState::Listening { port: port(52341) }, development, "MCP :52341"),
  ] {
    let listening = matches!(server, ServerState::Listening { .. });
    let mut bar = Bar::of(with(server, kind));
    assert_eq!(bar.texts("status-mcp"), Some(vec![words.to_owned()]));
    // Agents control the window only through a server that listens.
    assert_eq!(bar.texts("status-window-control").is_some(), listening, "{words}");
  }
}

#[test]
fn status_mcp_popover_names_the_address_the_agents_and_window_control() {
  let mut facts = with(ServerState::Listening { port: port(47821) }, McpKind::Installation);
  facts.mcp.clients = vec![
    client("claude-code", "2.1.296", true),
    client("codex-mcp-client", "0.159.2", true),
    client("gone", "1.0", false),
  ];
  let mut bar = Bar::of(facts);
  assert!(bar.texts("status-mcp-popover").is_none());
  bar.press("status-mcp");
  let popover = bar.texts("status-mcp-popover").expect("the popover opens");
  for words in [
    "Orchester MCP server",
    "Listening on 127.0.0.1:47821",
    "http://127.0.0.1:47821/mcp",
    "2 agents connected: claude-code, codex-mcp-client",
    "Agents can control this window",
    "On",
    "Open Automation settings",
  ] {
    assert!(popover.iter().any(|text| text == words), "{words} in {popover:?}");
  }
  bar.press("status-mcp-open-settings");
  assert_eq!(bar.asked(), vec![BarAction::Go(Route::Settings)]);
  assert!(bar.texts("status-mcp-popover").is_none(), "it closes");
}

#[test]
fn status_mcp_popover_says_why_agents_cannot_reach_it() {
  for (server, says) in [
    (ServerState::PortTaken { port: McpPort::DEFAULT }, "Port 47821 taken"),
    (ServerState::Off, "Off"),
    (ServerState::Unavailable, "Not running"),
  ] {
    let mut bar = Bar::of(with(server, McpKind::Installation));
    bar.press("status-mcp");
    let popover = bar.texts("status-mcp-popover").expect("the popover opens");
    assert!(popover.iter().any(|text| text == says), "{says} in {popover:?}");
    assert!(
      !popover.iter().any(|text| text.starts_with("http://")),
      "no address: {popover:?}"
    );
    assert!(bar.texts("status-mcp-window-control").is_none(), "{says}");
    assert!(bar.texts("status-mcp-open-settings").is_some(), "{says}");
  }
}

#[test]
fn status_mcp_popover_of_a_qa_copy_names_its_discovery_file_from_tmp_on() {
  let folder = PathBuf::from(ROOT).join(".tmp").join("qa").join("orc-31").join("mcp");
  let mut bar = Bar::of(with(
    ServerState::Listening { port: port(53122) },
    McpKind::Qa {
      folder: folder.clone(),
      file: Some(folder.join("53880.json")),
    },
  ));
  bar.press("status-mcp");
  let popover = bar.texts("status-mcp-popover").expect("the popover opens");
  let file = PathBuf::from(".tmp")
    .join("qa")
    .join("orc-31")
    .join("mcp")
    .join("53880.json");
  for words in ["Listening on 127.0.0.1:53122", file.display().to_string().as_str()] {
    assert!(popover.iter().any(|text| text == words), "{words} in {popover:?}");
  }
}
