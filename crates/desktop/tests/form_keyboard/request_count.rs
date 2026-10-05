//! A loopback port in front of the test server that counts the requests whose start line begins
//! with given texts, so a test can tell how often a screen asks the server.

use std::{
  io::{self, Read, Write},
  net::{Shutdown, SocketAddr, TcpListener, TcpStream},
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  thread,
};

/// Each counted start line with how often it was seen.
type Counts = Arc<Vec<(&'static str, AtomicUsize)>>;

pub(crate) struct RequestCount {
  port: u16,
  counts: Counts,
}

impl RequestCount {
  /// Forwards every connection to `target`, counting the requests that start with each of
  /// `starts`, e.g. `GET /api/v1/projects?`.
  pub(crate) fn start(target: SocketAddr, starts: &[&'static str]) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    let port = listener.local_addr().expect("address").port();
    let counts: Counts = Arc::new(starts.iter().map(|start| (*start, AtomicUsize::new(0))).collect());
    let counted = counts.clone();
    thread::spawn(move || {
      for client in listener.incoming().flatten() {
        // A connection that cannot be forwarded ends; the test then fails where it waits.
        let _ = forward(client, target, counted.clone());
      }
    });
    Self { port, counts }
  }

  pub(crate) fn url(&self) -> String {
    format!("http://127.0.0.1:{}", self.port)
  }

  /// How many requests started with `start`, one of the texts counted.
  pub(crate) fn count(&self, start: &str) -> usize {
    self
      .counts
      .iter()
      .find(|(counted, _)| *counted == start)
      .map(|(_, count)| count.load(Ordering::SeqCst))
      .expect("a counted start line")
  }
}

fn forward(client: TcpStream, target: SocketAddr, counts: Counts) -> io::Result<()> {
  let server = TcpStream::connect(target)?;
  let (mut client_in, mut server_out) = (client.try_clone()?, server.try_clone()?);
  let (mut server_in, mut client_out) = (server, client);
  let longest = counts.iter().map(|(start, _)| start.len()).max().unwrap_or_default();
  thread::spawn(move || {
    // What was read last, so a start line split between two reads is still found once.
    let mut seen: Vec<u8> = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
      let read = match client_in.read(&mut buffer) {
        Ok(0) | Err(_) => break,
        Ok(read) => read,
      };
      let kept = seen.len();
      seen.extend_from_slice(&buffer[..read]);
      for (start, count) in counts.iter() {
        let start = start.as_bytes();
        // Only matches that end in what was just read: the kept tail is shorter than any text.
        let found = seen
          .windows(start.len())
          .enumerate()
          .filter(|(at, window)| *window == start && at + start.len() > kept)
          .count();
        count.fetch_add(found, Ordering::SeqCst);
      }
      let keep = seen.len().min(longest.saturating_sub(1));
      seen.drain(..seen.len() - keep);
      if server_out.write_all(&buffer[..read]).is_err() {
        break;
      }
    }
    // Either side closing ends the copy; that is how a connection finishes.
    let _ = server_out.shutdown(Shutdown::Write);
  });
  thread::spawn(move || {
    let _ = io::copy(&mut server_in, &mut client_out);
    let _ = client_out.shutdown(Shutdown::Write);
  });
  Ok(())
}
