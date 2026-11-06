//! The chat's domain tools as a stand-in on loopback, for the turns that call them.

use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpListener,
};

/// The chat's domain tools as a stand-in on loopback: it answers MCP's `initialize` and every call, as the app's
/// server for a turn does, and keeps each call's `Authorization` header, tool and arguments.
pub(crate) struct ToolServer {
  pub(crate) url: String,
  calls: Arc<Mutex<Vec<(String, String, Value)>>>,
}

impl ToolServer {
  pub(crate) async fn start() -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback port");
    let url = format!("http://{}/mcp", listener.local_addr().expect("its address"));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let kept = calls.clone();
    tokio::spawn(async move {
      while let Ok((mut stream, _)) = listener.accept().await {
        let kept = kept.clone();
        tokio::spawn(async move {
          let (authorization, request) = read_request(&mut stream).await;
          let response = answer(&request, &authorization, &kept);
          let written = stream.write_all(response.as_bytes()).await;
          assert!(written.is_ok(), "the fake CLI reads the answer");
        });
      }
    });
    Self { url, calls }
  }

  pub(crate) fn calls(&self) -> Vec<(String, String, Value)> {
    self.calls.lock().clone()
  }
}

/// One HTTP request: its `Authorization` header and its JSON body.
async fn read_request(stream: &mut tokio::net::TcpStream) -> (String, Value) {
  let mut read = Vec::new();
  let mut buffer = [0_u8; 4096];
  let (head, body_start) = loop {
    let count = stream.read(&mut buffer).await.expect("the request");
    assert!(count > 0, "the request ended early");
    read.extend_from_slice(&buffer[..count]);
    if let Some(at) = read.windows(4).position(|window| window == b"\r\n\r\n") {
      break (String::from_utf8_lossy(&read[..at]).into_owned(), at + 4);
    }
  };
  let header = |name: &str| {
    head
      .lines()
      .find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim().to_owned())
      })
      .unwrap_or_default()
  };
  let length: usize = header("content-length").parse().unwrap_or(0);
  while read.len() < body_start + length {
    let count = stream.read(&mut buffer).await.expect("the body");
    assert!(count > 0, "the body ended early");
    read.extend_from_slice(&buffer[..count]);
  }
  let body = serde_json::from_slice(&read[body_start..body_start + length]).unwrap_or(Value::Null);
  (header("authorization"), body)
}

/// The HTTP answer to `request`: `initialize`'s with a session, a call's with its result, a notification's empty.
fn answer(request: &Value, authorization: &str, calls: &Mutex<Vec<(String, String, Value)>>) -> String {
  let result = match request["method"].as_str() {
    Some("initialize") => {
      json!({"protocolVersion": "2025-03-26", "capabilities": {"tools": {}}, "serverInfo": {"name": "orchester-orchestrator", "version": "1"}})
    }
    Some("tools/call") => {
      let name = request["params"]["name"].as_str().unwrap_or_default().to_owned();
      calls
        .lock()
        .push((authorization.to_owned(), name, request["params"]["arguments"].clone()));
      json!({"content": [{"type": "text", "text": "Created KON-1 as a draft."}]})
    }
    _ => return "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(),
  };
  let body = json!({"jsonrpc": "2.0", "id": request["id"], "result": result}).to_string();
  format!(
    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nMcp-Session-Id: chat-session\r\nContent-Length: {}\r\n\
     Connection: close\r\n\r\n{body}",
    body.len()
  )
}
