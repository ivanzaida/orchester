//! A minimal SMTP relay on loopback that accepts every message and keeps its data, for the SMTP mailer's test.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use parking_lot::Mutex;
use tokio::{
  io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
  net::{TcpListener, TcpStream},
};

#[derive(Clone, Default)]
pub struct Relay {
  messages: Arc<Mutex<Vec<String>>>,
}

impl Relay {
  pub async fn start() -> (Self, SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("relay listens");
    let address = listener.local_addr().expect("relay address");
    let relay = Self::default();
    let messages = relay.messages.clone();
    tokio::spawn(async move {
      while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(session(stream, messages.clone()));
      }
    });
    (relay, address)
  }

  pub async fn first(&self) -> String {
    tokio::time::timeout(Duration::from_secs(10), async {
      loop {
        if let Some(message) = self.messages.lock().first() {
          return message.clone();
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
      }
    })
    .await
    .expect("the relay receives the email")
  }
}

async fn session(stream: TcpStream, messages: Arc<Mutex<Vec<String>>>) {
  let (read, mut write) = stream.into_split();
  let mut lines = BufReader::new(read).lines();
  write.write_all(b"220 relay.test ESMTP\r\n").await.expect("greeting");
  while let Ok(Some(line)) = lines.next_line().await {
    let command = line.to_ascii_uppercase();
    let reply: &[u8] = if command.starts_with("EHLO") || command.starts_with("HELO") {
      b"250-relay.test\r\n250 8BITMIME\r\n"
    } else if command.starts_with("DATA") {
      write.write_all(b"354 go ahead\r\n").await.expect("data reply");
      let mut data = String::new();
      while let Ok(Some(line)) = lines.next_line().await {
        if line == "." {
          break;
        }
        data.push_str(&line);
        data.push('\n');
      }
      messages.lock().push(data);
      b"250 queued\r\n"
    } else if command.starts_with("QUIT") {
      write.write_all(b"221 bye\r\n").await.expect("quit reply");
      return;
    } else {
      b"250 ok\r\n"
    };
    write.write_all(reply).await.expect("reply");
  }
}
