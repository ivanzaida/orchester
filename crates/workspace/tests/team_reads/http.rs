//! The fake team server's HTTP: one request read from a connection, a JSON answer, and an `AuthorityError` refusal.

use std::{
  io::{BufRead, BufReader, Read, Write},
  net::TcpStream,
};

use serde_json::{Value, json};

/// One request: its method, path, headers with lower-cased names, and body.
pub struct Request {
  pub method: String,
  pub path: String,
  headers: Vec<(String, String)>,
  body: Vec<u8>,
}

impl Request {
  pub fn read(stream: &TcpStream) -> Option<Self> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next()?.to_owned(), parts.next()?.to_owned());
    let mut headers = Vec::new();
    loop {
      let mut header = String::new();
      reader.read_line(&mut header).ok()?;
      let header = header.trim_end();
      if header.is_empty() {
        break;
      }
      let (name, value) = header.split_once(':')?;
      headers.push((name.to_ascii_lowercase(), value.trim().to_owned()));
    }
    let length = headers
      .iter()
      .find(|(name, _)| name == "content-length")
      .and_then(|(_, value)| value.parse().ok())
      .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Self {
      method,
      path,
      headers,
      body,
    })
  }

  pub fn header(&self, name: &str) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|(header, _)| header == name)
      .map(|(_, value)| value.as_str())
  }

  pub fn token(&self) -> Option<&str> {
    self.header("authorization")?.strip_prefix("Bearer ")
  }

  /// The body as `T`, which also checks it is what the contract sends.
  pub fn json<T: serde::de::DeserializeOwned>(&self) -> T {
    serde_json::from_slice(&self.body).expect("a request the contract reads")
  }
}

pub fn respond(stream: &mut TcpStream, status: u16, body: &Value) {
  let body = body.to_string();
  let head = format!(
    "HTTP/1.1 {status} Answer\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
    body.len()
  );
  // A client that hung up learns nothing; the test reads what reached it.
  let _ = stream
    .write_all(head.as_bytes())
    .and_then(|()| stream.write_all(body.as_bytes()));
}

pub fn refusal(stream: &mut TcpStream, status: u16, code: &str) {
  respond(stream, status, &json!({ "code": code, "details": {} }));
}

/// `event` as one SSE event: its kind as the name, its cursor as the id, itself as the data.
pub fn sse(event: &Value) -> String {
  let id = event
    .get("cursor")
    .and_then(Value::as_str)
    .map(|cursor| format!("id: {cursor}\n"))
    .unwrap_or_default();
  format!(
    "{id}event: {}\ndata: {event}\n\n",
    event["kind"].as_str().expect("a kind")
  )
}
