//! The fake team server's HTTP: one request read from a connection and a JSON answer.

use std::{
  io::{BufRead, BufReader, Read, Write},
  net::TcpStream,
};

use serde_json::Value;

/// One request: its method, path, bearer token and body.
pub struct Request {
  pub method: String,
  pub path: String,
  pub token: String,
  pub body: Vec<u8>,
}

impl Request {
  pub fn read(stream: &TcpStream) -> Option<Self> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next()?.to_owned(), parts.next()?.to_owned());
    let (mut token, mut length) = (String::new(), 0);
    loop {
      let mut header = String::new();
      reader.read_line(&mut header).ok()?;
      let header = header.trim_end();
      if header.is_empty() {
        break;
      }
      let (name, value) = header.split_once(':')?;
      match name.to_ascii_lowercase().as_str() {
        "content-length" => length = value.trim().parse().ok()?,
        "authorization" => value.trim().strip_prefix("Bearer ")?.clone_into(&mut token),
        _ => {}
      }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Self {
      method,
      path,
      token,
      body,
    })
  }

  pub fn json(&self) -> Value {
    serde_json::from_slice(&self.body).expect("a JSON request")
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
