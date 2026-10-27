//! Where the client sends its requests (`docs/cloud-api.md`, "Base"): an Orchester Cloud with an API address of its own
//! at `/v1` there, one without at `/api/v1` on its web address, and a team server at `/api/v1` under its address.
//!
//! `cargo test -p orchester-api --features client --test cloud_address cloud_address_`

use std::{
  io::{Read, Write},
  net::TcpListener,
  sync::{Arc, Mutex},
  thread,
};

use orchester_api::client::{Client, ClientError, CloudAddress, CloudClient, PlainHttp, ServerAddress};

/// A listener on loopback that answers every request `404` and keeps its request lines, e.g. `GET /v1/keys`.
struct Recorder {
  url: String,
  lines: Arc<Mutex<Vec<String>>>,
}

impl Recorder {
  fn start() -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    let url = format!("http://localhost:{}", listener.local_addr().expect("address").port());
    let lines = Arc::new(Mutex::new(Vec::new()));
    let kept = lines.clone();
    thread::spawn(move || {
      for mut stream in listener.incoming().flatten() {
        let mut request = [0_u8; 4096];
        let read = stream.read(&mut request).unwrap_or(0);
        let text = String::from_utf8_lossy(&request[..read]);
        let line = text.lines().next().unwrap_or_default();
        let line = line.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
        kept.lock().expect("lines").push(line);
        // The test reads the lines; whether the client read the answer does not matter.
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
      }
    });
    Self { url, lines }
  }

  fn lines(&self) -> Vec<String> {
    self.lines.lock().expect("lines").clone()
  }
}

fn address(text: &str) -> ServerAddress {
  ServerAddress::parse(text).expect("address")
}

#[tokio::test]
async fn cloud_address_with_an_api_address_calls_v1_there() {
  let api = Recorder::start();
  let cloud = CloudAddress::new(address("https://orchester.cloud"), Some(address(&api.url)));
  assert_eq!(cloud.api_base(), format!("{}/v1", api.url));
  let client = CloudClient::new(cloud).expect("client");
  assert!(client.key_set().await.is_err(), "the recorder answers 404");
  assert_eq!(api.lines(), ["GET /v1/keys"]);
}

#[tokio::test]
async fn cloud_address_without_an_api_address_calls_api_v1_on_the_web_address() {
  let web = Recorder::start();
  let cloud = CloudAddress::new(address(&web.url), None);
  assert_eq!(cloud.api_host(), cloud.web());
  assert_eq!(cloud.api_base(), format!("{}/api/v1", web.url));
  let client = CloudClient::new(cloud).expect("client");
  assert!(client.key_set().await.is_err(), "the recorder answers 404");
  assert_eq!(web.lines(), ["GET /api/v1/keys"]);
}

#[tokio::test]
async fn cloud_address_leaves_a_team_server_at_api_v1() {
  let server = Recorder::start();
  let client = Client::new(address(&server.url), PlainHttp::NotAllowed).expect("client");
  assert!(client.server_info().await.is_err(), "the recorder answers 404");
  assert_eq!(server.lines(), ["GET /api/v1/server"]);
}

#[test]
fn cloud_address_with_a_path_prefix_keeps_it() {
  let cloud = CloudAddress::new(
    address("https://kontur.lan/cloud"),
    Some(address("https://api.kontur.lan/cloud/")),
  );
  assert_eq!(cloud.api_base(), "https://api.kontur.lan/cloud/v1");
  assert_eq!(cloud.web().as_str(), "https://kontur.lan/cloud");
}

#[test]
fn cloud_address_refuses_plain_http_to_an_api_address_on_another_computer() {
  let cloud = CloudAddress::new(address("http://localhost:8080"), Some(address("http://api.kontur.lan")));
  assert!(matches!(CloudClient::new(cloud), Err(ClientError::PlainHttpRefused)));
}
