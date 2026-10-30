//! The fake team server's HTTP: one request read from a connection, a JSON answer or refusal, and an event as SSE.

use std::{
  io::{BufRead, BufReader, Read, Write},
  net::TcpStream,
};

use orchester_api::Event;
use serde_json::{Value, json};

/// `event` as one SSE event: its kind as the name, its payload as the data.
pub fn sse(id: Option<u64>, event: &Event) -> String {
  let value = serde_json::to_value(event).expect("an event serializes");
  let id = id.map(|id| format!("id: {id}\n")).unwrap_or_default();
  format!(
    "{id}event: {}\ndata: {}\n\n",
    value["kind"].as_str().expect("a kind"),
    value["data"]
  )
}

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

  pub fn json<T: serde::de::DeserializeOwned>(&self) -> T {
    serde_json::from_slice(&self.body).expect("a request the contract reads")
  }

  pub fn bytes(&self) -> &[u8] {
    &self.body
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

pub fn refusal(stream: &mut TcpStream, status: u16, error: &str) {
  respond(stream, status, &json!({ "error": error, "message": "Refused" }));
}

/// `204`: a success without a body.
pub fn no_content(stream: &mut TcpStream) {
  // A client that hung up learns nothing; the test reads what reached it.
  let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n");
}
