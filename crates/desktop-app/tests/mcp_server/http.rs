//! Just enough HTTP/1.1 and streamable-HTTP MCP to talk to the server on loopback from a test.

use std::{
  io::{Read, Write},
  net::TcpStream,
  time::Duration,
};

use serde_json::Value;

pub struct Response {
  pub status: u16,
  pub session: Option<String>,
  /// The JSON-RPC message of the answer, from a JSON body or the first `data:` event.
  pub message: Option<Value>,
  pub text: String,
}

/// Posts `body` to `/mcp` on `port` with the extra `headers` (`Authorization`, `Mcp-Session-Id`,
/// `Origin`).
pub fn post(port: u16, headers: &[(&str, &str)], body: &Value) -> Response {
  let body = body.to_string();
  let mut request = format!(
    "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nAccept: application/json, \
     text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n",
    body.len()
  );
  for (name, value) in headers {
    request.push_str(&format!("{name}: {value}\r\n"));
  }
  request.push_str("\r\n");
  request.push_str(&body);
  let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("the server accepts connections");
  stream
    .set_read_timeout(Some(Duration::from_secs(30)))
    .expect("a read timeout can be set");
  stream.write_all(request.as_bytes()).expect("the request is sent");
  let mut raw = Vec::new();
  stream.read_to_end(&mut raw).expect("the response arrives");
  parse(&String::from_utf8_lossy(&raw))
}

fn parse(raw: &str) -> Response {
  let (head, body) = raw.split_once("\r\n\r\n").expect("a response has a head and a body");
  let status = head
    .split_whitespace()
    .nth(1)
    .and_then(|code| code.parse().ok())
    .expect("a status line");
  let header = |name: &str| {
    head.lines().find_map(|line| {
      let (key, value) = line.split_once(':')?;
      key.trim().eq_ignore_ascii_case(name).then(|| value.trim().to_owned())
    })
  };
  let text = if header("transfer-encoding").is_some_and(|value| value.eq_ignore_ascii_case("chunked")) {
    dechunk(body)
  } else {
    body.to_owned()
  };
  let message = serde_json::from_str(&text).ok().or_else(|| {
    text
      .lines()
      .filter_map(|line| line.strip_prefix("data:"))
      .find_map(|data| serde_json::from_str(data.trim()).ok())
  });
  Response {
    status,
    session: header("mcp-session-id"),
    message,
    text,
  }
}

fn dechunk(body: &str) -> String {
  let mut rest = body;
  let mut out = String::new();
  while let Some((size, after)) = rest.split_once("\r\n") {
    let Ok(size) = usize::from_str_radix(size.trim(), 16) else {
      break;
    };
    if size == 0 || after.len() < size {
      break;
    }
    out.push_str(&after[..size]);
    rest = after[size..].trim_start_matches("\r\n");
  }
  out
}
