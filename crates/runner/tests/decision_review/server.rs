use std::{sync::Arc, time::Duration};

use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpListener,
  task::JoinHandle,
};

pub struct Server {
  pub url: String,
  pub requests: Arc<Mutex<Vec<String>>>,
  task: JoinHandle<()>,
}

impl Server {
  pub async fn new(status: &str, body: String, delay: Duration, listed: bool) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("address"));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let received = requests.clone();
    let status = status.to_owned();
    let task = tokio::spawn(async move {
      loop {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut bytes = Vec::new();
        loop {
          let mut chunk = [0; 4096];
          let n = stream.read(&mut chunk).await.expect("request");
          if n == 0 {
            break;
          }
          bytes.extend_from_slice(&chunk[..n]);
          if let Some(at) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            let header = String::from_utf8_lossy(&bytes[..at]).to_lowercase();
            let size: usize = header
              .lines()
              .find_map(|line| line.strip_prefix("content-length: "))
              .map_or(0, |size| size.parse().expect("size"));
            if bytes.len() >= at + 4 + size {
              break;
            }
          }
        }
        let request = String::from_utf8(bytes).expect("UTF-8");
        let catalog = request.starts_with("GET ");
        received.lock().push(request);
        let catalog_body = json!({"models": if listed { vec![json!({"name": "jev-latest"})] } else { vec![] }});
        let (status, body) = if catalog {
          ("200 OK", catalog_body.to_string())
        } else {
          tokio::time::sleep(delay).await;
          (status.as_str(), body.clone())
        };
        let response = format!(
          "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
          body.len()
        );
        // Timeouts deliberately close the peer before this response is sent.
        if stream.write_all(response.as_bytes()).await.is_err() {
          continue;
        }
      }
    });
    Self { url, requests, task }
  }
}

impl Drop for Server {
  fn drop(&mut self) {
    self.task.abort();
  }
}

pub fn answer(approve: f64) -> Value {
  let mut value: Value =
    serde_json::from_str(include_str!("../fixtures/system_one/documentation-derived-jev.json")).expect("fixture");
  value["answers"]["verdict"]["probabilities"] = json!({"approve": approve, "ask": 1.0 - approve});
  value
}
