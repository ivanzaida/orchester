use std::{
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  time::Duration,
};

use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::{TcpListener, TcpStream},
  task::{JoinHandle, JoinSet},
};

pub struct Server {
  pub url: String,
  pub bodies: Arc<Mutex<Vec<Value>>>,
  pub peak: Arc<AtomicUsize>,
  task: JoinHandle<()>,
}

impl Server {
  pub async fn new(delay: Duration, malformed: bool) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("address"));
    let bodies = Arc::new(Mutex::new(vec![]));
    let peak = Arc::new(AtomicUsize::new(0));
    let (received, highest) = (bodies.clone(), peak.clone());
    let task = tokio::spawn(async move {
      let active = Arc::new(AtomicUsize::new(0));
      let mut workers = JoinSet::new();
      loop {
        tokio::select! {
          peer = listener.accept() => {
            let (peer, _) = peer.expect("accept");
            let (bodies, active, peak) = (received.clone(), active.clone(), highest.clone());
            workers.spawn(serve(peer, bodies, active, peak, delay, malformed));
          }
          _ = workers.join_next(), if !workers.is_empty() => {}
        }
      }
    });
    Self {
      url,
      bodies,
      peak,
      task,
    }
  }
}
impl Drop for Server {
  fn drop(&mut self) {
    self.task.abort();
  }
}

async fn serve(
  mut stream: TcpStream,
  bodies: Arc<Mutex<Vec<Value>>>,
  active: Arc<AtomicUsize>,
  peak: Arc<AtomicUsize>,
  delay: Duration,
  malformed: bool,
) {
  let mut bytes = Vec::new();
  let at = loop {
    let mut chunk = [0; 4096];
    let n = stream.read(&mut chunk).await.expect("request");
    if n == 0 {
      return;
    }
    bytes.extend_from_slice(&chunk[..n]);
    if let Some(at) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
      let header = String::from_utf8_lossy(&bytes[..at]).to_lowercase();
      let size = header
        .lines()
        .find_map(|line| line.strip_prefix("content-length: "))
        .map_or(0, |size| size.parse::<usize>().expect("size"));
      if bytes.len() >= at + 4 + size {
        break at;
      }
    }
  };
  let catalog = bytes.starts_with(b"GET ");
  let body = if catalog {
    json!({"models": [{"name": "jev-latest"}]}).to_string()
  } else {
    let input: Value = serde_json::from_slice(&bytes[at + 4..]).expect("body");
    bodies.lock().push(input);
    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
    peak.fetch_max(current, Ordering::SeqCst);
    tokio::time::sleep(delay).await;
    active.fetch_sub(1, Ordering::SeqCst);
    if malformed {
      "{}".to_owned()
    } else {
      let mut answer: Value =
        serde_json::from_str(include_str!("../fixtures/system_one/documentation-derived-jev.json")).expect("fixture");
      answer["answers"]["verdict"]["probabilities"] = json!({"approve": 0.9, "ask": 0.1});
      answer.to_string()
    }
  };
  let response = format!(
    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
    body.len()
  );
  // Stop deliberately closes in-flight sockets, leaving no recipient for the response.
  let _ = stream.write_all(response.as_bytes()).await;
}
