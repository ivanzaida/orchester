use std::{collections::HashMap, net::SocketAddr, time::Duration};

use serde::de::DeserializeOwned;
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpStream,
};

pub struct Response {
  pub status: u16,
  pub headers: HashMap<String, String>,
  pub body: Vec<u8>,
}

impl Response {
  pub fn json<T: DeserializeOwned>(&self) -> T {
    serde_json::from_slice(&self.body).unwrap_or_else(|_| panic!("response {} did not match the contract", self.status))
  }

  pub fn error(&self) -> String {
    self.json::<serde_json::Value>()["error"]
      .as_str()
      .expect("an error code")
      .to_owned()
  }
}

/// One HTTP/1.1 request on its own connection, so headers and bodies are exactly what the test wrote.
pub async fn exchange(
  address: SocketAddr,
  method: &str,
  path: &str,
  headers: &[(&str, &str)],
  body: &[u8],
) -> Response {
  let mut stream = TcpStream::connect(address).await.expect("connect to the cloud");
  let mut request = format!(
    "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n",
    body.len()
  );
  for (name, value) in headers {
    request.push_str(&format!("{name}: {value}\r\n"));
  }
  request.push_str("\r\n");
  stream
    .write_all(request.as_bytes())
    .await
    .expect("send the request head");
  stream.write_all(body).await.expect("send the request body");
  let mut response = Vec::new();
  tokio::time::timeout(Duration::from_secs(30), stream.read_to_end(&mut response))
    .await
    .expect("response deadline")
    .expect("read the response");
  parse(&response)
}

fn parse(response: &[u8]) -> Response {
  let boundary = response
    .windows(4)
    .position(|part| part == b"\r\n\r\n")
    .expect("an HTTP response");
  let head = std::str::from_utf8(&response[..boundary]).expect("HTTP head");
  let mut lines = head.split("\r\n");
  let status = lines
    .next()
    .and_then(|line| line.split_whitespace().nth(1))
    .and_then(|code| code.parse().ok())
    .expect("a status code");
  let headers: HashMap<String, String> = lines
    .filter_map(|line| line.split_once(':'))
    .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
    .collect();
  let mut body = response[boundary + 4..].to_vec();
  if headers.get("transfer-encoding").is_some_and(|value| value == "chunked") {
    body = dechunk(&body);
  }
  Response { status, headers, body }
}

fn dechunk(mut body: &[u8]) -> Vec<u8> {
  let mut out = Vec::new();
  loop {
    let line = body.windows(2).position(|part| part == b"\r\n").expect("chunk size");
    let size = usize::from_str_radix(std::str::from_utf8(&body[..line]).expect("hex").trim(), 16).expect("size");
    if size == 0 {
      return out;
    }
    out.extend_from_slice(&body[line + 2..line + 2 + size]);
    body = &body[line + 4 + size..];
  }
}
