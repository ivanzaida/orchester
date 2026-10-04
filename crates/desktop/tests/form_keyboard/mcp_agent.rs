//! An agent of the embedded MCP server, as a coding agent drives the app: JSON-RPC posted over
//! loopback HTTP with the server's bearer token, replies read as JSON or as server-sent events.

use std::{
  io::{BufRead, BufReader, Read, Write},
  net::TcpStream,
  time::Duration,
};

use serde_json::{Value, json};

/// Longest wait for one reply; the harness answers between its passes.
const REPLY_TIMEOUT: Duration = Duration::from_secs(60);

pub struct McpAgent {
  port: u16,
  token: String,
  session: Option<String>,
  next_id: u64,
}

impl McpAgent {
  /// Connects and initializes a session.
  pub fn connect(port: u16, token: String) -> Self {
    let mut agent = Self {
      port,
      token,
      session: None,
      next_id: 0,
    };
    agent.request(
      "initialize",
      &json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "orchester-test", "version": "0"}}),
    );
    agent.post(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    agent
  }

  /// Calls a tool and returns its text content.
  pub fn tool(&mut self, name: &str, arguments: &Value) -> String {
    let result = self.request("tools/call", &json!({"name": name, "arguments": arguments}));
    assert_ne!(result["isError"], true, "{name} failed: {result}");
    result["content"][0]["text"].as_str().unwrap_or_default().to_owned()
  }

  fn request(&mut self, method: &str, params: &Value) -> Value {
    self.next_id += 1;
    let reply = self
      .post(&json!({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params}))
      .expect("a request gets a reply");
    reply
      .get("result")
      .cloned()
      .unwrap_or_else(|| panic!("{method} failed: {reply}"))
  }

  /// Posts `body`; returns the reply to its `id`, or `None` for a notification.
  fn post(&mut self, body: &Value) -> Option<Value> {
    let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("the MCP server accepts a connection");
    stream.set_read_timeout(Some(REPLY_TIMEOUT)).expect("a read timeout");
    let payload = body.to_string();
    let session = self
      .session
      .as_ref()
      .map(|id| format!("mcp-session-id: {id}\r\n"))
      .unwrap_or_default();
    write!(
      stream,
      "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\n\
       Accept: application/json, text/event-stream\r\n{session}Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
      self.port,
      self.token,
      payload.len()
    )
    .expect("the request is sent");
    let mut reader = BufReader::new(stream);
    let chunked = self.read_headers(&mut reader);
    let wanted = body.get("id")?.clone();
    let mut received = Vec::new();
    loop {
      read_body(&mut reader, chunked, &mut received);
      let text = String::from_utf8_lossy(&received);
      let reply = text
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .chain(text.trim_start().starts_with('{').then_some(text.as_ref()))
        .filter_map(|data| serde_json::from_str::<Value>(data.trim()).ok())
        .find(|message| message.get("id") == Some(&wanted));
      if reply.is_some() {
        return reply;
      }
      assert!(chunked, "no JSON-RPC reply in {text}");
    }
  }

  /// Reads the status and headers, keeping the session; whether the body comes in chunks.
  fn read_headers(&mut self, reader: &mut BufReader<TcpStream>) -> bool {
    let mut line = String::new();
    reader.read_line(&mut line).expect("a status line");
    assert!(line.contains(" 200 ") || line.contains(" 202 "), "HTTP status: {line}");
    let mut chunked = false;
    loop {
      line.clear();
      reader.read_line(&mut line).expect("a header line");
      let Some((name, value)) = line.trim_end().split_once(':') else {
        return chunked;
      };
      let (name, value) = (name.to_ascii_lowercase(), value.trim());
      if name == "mcp-session-id" {
        self.session = Some(value.to_owned());
      } else if name == "transfer-encoding" && value.eq_ignore_ascii_case("chunked") {
        chunked = true;
      }
    }
  }
}

/// Appends the next chunk of a chunked body, or the whole of any other body, to `received`.
fn read_body(reader: &mut BufReader<TcpStream>, chunked: bool, received: &mut Vec<u8>) {
  if !chunked {
    reader.read_to_end(received).expect("the body");
    return;
  }
  let mut line = String::new();
  reader.read_line(&mut line).expect("a chunk size");
  let size = usize::from_str_radix(line.trim(), 16).expect("a hexadecimal chunk size");
  assert_ne!(size, 0, "the stream ended without a reply");
  let mut chunk = vec![0; size + 2];
  reader.read_exact(&mut chunk).expect("a chunk");
  received.extend_from_slice(&chunk[..size]);
}
