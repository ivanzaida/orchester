//! A loopback port in front of the test server that counts the requests whose start line begins
//! with a given text, so a test can tell how often a screen asks the server.

use std::{
  io::{self, Read, Write},
  net::{Shutdown, SocketAddr, TcpListener, TcpStream},
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  thread,
};

pub(crate) struct RequestCount {
  port: u16,
  count: Arc<AtomicUsize>,
}

impl RequestCount {
  /// Forwards every connection to `target`, counting the requests that start with `start`, e.g.
  /// `GET /api/v1/projects?`.
  pub(crate) fn start(target: SocketAddr, start: &'static str) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    let port = listener.local_addr().expect("address").port();
    let count = Arc::new(AtomicUsize::new(0));
    let counted = count.clone();
    thread::spawn(move || {
      for client in listener.incoming().flatten() {
        // A connection that cannot be forwarded ends; the test then fails where it waits.
        let _ = forward(client, target, (start.as_bytes(), counted.clone()));
      }
    });
    Self { port, count }
  }

  pub(crate) fn url(&self) -> String {
    format!("http://127.0.0.1:{}", self.port)
  }

  pub(crate) fn count(&self) -> usize {
    self.count.load(Ordering::SeqCst)
  }
}

fn forward(client: TcpStream, target: SocketAddr, (start, count): (&'static [u8], Arc<AtomicUsize>)) -> io::Result<()> {
  let server = TcpStream::connect(target)?;
  let (mut client_in, mut server_out) = (client.try_clone()?, server.try_clone()?);
  let (mut server_in, mut client_out) = (server, client);
  thread::spawn(move || {
    // What was read last, so a start line split between two reads is still found.
    let mut seen: Vec<u8> = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
      let read = match client_in.read(&mut buffer) {
        Ok(0) | Err(_) => break,
        Ok(read) => read,
      };
      seen.extend_from_slice(&buffer[..read]);
      let found = seen.windows(start.len()).filter(|window| *window == start).count();
      count.fetch_add(found, Ordering::SeqCst);
      let keep = seen.len().min(start.len().saturating_sub(1));
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
