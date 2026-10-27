//! Agents connected to the installed app's MCP server in the QA app's `settings` scenario, so *Agents connected*
//! lists them (`04 · Automation` `aMkOw`): two fixture clients that name themselves as Claude Code and Codex do in
//! their `initialize`, then keep their sessions open with a ping every half minute. They speak MCP's streamable HTTP
//! over loopback with the token that server stored in its keychain in memory; nothing leaves this computer.

use std::{
  io::{self, Read, Write},
  net::TcpStream,
  sync::Arc,
  thread,
  time::{Duration, Instant},
};

use serde_json::{Value, json};

use super::signed_in::MemoryKeychain;

/// The names and versions the clients give, as Claude Code's and Codex's own do.
const CLIENTS: [(&str, &str); 2] = [("claude-code", "2.1.296"), ("codex-mcp-client", "0.159.2")];
/// How often each client asks something, so its session stays open.
const PING_EVERY: Duration = Duration::from_secs(30);

/// Connects the clients to the server at `port` once it stored its token in `keychain`.
pub fn connect(port: u16, keychain: Arc<MemoryKeychain>) {
  thread::spawn(move || {
    let deadline = Instant::now() + Duration::from_secs(60);
    let token = loop {
      if let Some(token) = keychain.values().into_iter().next() {
        break token;
      }
      if Instant::now() > deadline {
        println!("fixture agents: the installed app's server stored no token");
        return;
      }
      thread::sleep(Duration::from_millis(100));
    };
    let sessions: Vec<String> = CLIENTS
      .iter()
      .filter_map(|client| match open(port, &token, *client) {
        Ok(session) => Some(session),
        Err(error) => {
          println!("fixture agent {} did not connect: {error}", client.0);
          None
        }
      })
      .collect();
    for ping in 1_u64.. {
      thread::sleep(PING_EVERY);
      for session in &sessions {
        let request = json!({ "jsonrpc": "2.0", "id": ping + 1, "method": "ping" });
        if let Err(error) = post(port, &token, Some(session), &request) {
          println!("fixture agent's ping failed: {error}");
        }
      }
    }
  });
}

/// Initializes a session as `client` names itself; its id.
fn open(port: u16, token: &str, (name, version): (&str, &str)) -> io::Result<String> {
  let initialize = json!({
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2025-06-18",
      "capabilities": {},
      "clientInfo": { "name": name, "version": version },
    },
  });
  let headers = post(port, token, None, &initialize)?;
  let session = headers
    .lines()
    .find_map(|line| {
      let (header, value) = line.split_once(':')?;
      header
        .eq_ignore_ascii_case("mcp-session-id")
        .then(|| value.trim().to_owned())
    })
    .ok_or_else(|| io::Error::other(format!("no session in {headers}")))?;
  let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
  post(port, token, Some(&session), &initialized)?;
  Ok(session)
}

/// Posts `body` to the server's endpoint; the answer's status line and headers.
fn post(port: u16, token: &str, session: Option<&str>, body: &Value) -> io::Result<String> {
  let body = body.to_string();
  let session = session.map_or_else(String::new, |session| format!("Mcp-Session-Id: {session}\r\n"));
  let mut stream = TcpStream::connect(("127.0.0.1", port))?;
  stream.set_read_timeout(Some(Duration::from_secs(5)))?;
  write!(
    stream,
    "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\n{session}Content-Type: \
     application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: \
     close\r\n\r\n{body}",
    body.len()
  )?;
  let mut answer = vec![0_u8; 4096];
  let read = stream.read(&mut answer)?;
  let text = String::from_utf8_lossy(&answer[..read]).into_owned();
  let head = text.split("\r\n\r\n").next().unwrap_or_default().to_owned();
  if !head.starts_with("HTTP/1.1 2") {
    return Err(io::Error::other(head));
  }
  Ok(head)
}
