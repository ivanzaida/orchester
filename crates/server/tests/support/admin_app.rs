//! A stand-in for the server admin app on loopback: it answers every request with a page and keeps each request's
//! head, so a test can see what the team server forwarded.

use std::{net::SocketAddr, sync::Arc};

use parking_lot::Mutex;
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpListener,
};

pub struct AdminApp {
  pub url: String,
  heads: Arc<Mutex<Vec<String>>>,
}

impl AdminApp {
  pub async fn start() -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listen");
    let address: SocketAddr = listener.local_addr().expect("address");
    let heads = Arc::new(Mutex::new(Vec::new()));
    let kept = heads.clone();
    tokio::spawn(async move {
      while let Ok((mut stream, _)) = listener.accept().await {
        let kept = kept.clone();
        tokio::spawn(async move {
          let mut buffer = vec![0_u8; 16 * 1024];
          let mut read = 0;
          while !buffer[..read].windows(4).any(|part| part == b"\r\n\r\n") {
            match stream.read(&mut buffer[read..]).await {
              Ok(0) | Err(_) => return,
              Ok(count) => read += count,
            }
          }
          kept.lock().push(String::from_utf8_lossy(&buffer[..read]).into_owned());
          let body = "admin page";
          let answer = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nX-Admin-App: yes\r\nKeep-Alive: timeout=5\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
          );
          stream.write_all(answer.as_bytes()).await.expect("answer");
        });
      }
    });
    Self {
      url: format!("http://{address}"),
      heads,
    }
  }

  /// The head of every request the app received, in order.
  pub fn heads(&self) -> Vec<String> {
    self.heads.lock().clone()
  }
}
