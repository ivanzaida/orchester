//! A loopback port in front of the test server that can be taken down and brought back, as a
//! server that goes away and returns: while down it closes every connection at once, the open
//! ones included, so nothing reaches the server.

use std::{
  io,
  net::{Shutdown, SocketAddr, TcpListener, TcpStream},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
};

use parking_lot::Mutex;

pub struct Forwarder {
  port: u16,
  up: Arc<AtomicBool>,
  open: Arc<Mutex<Vec<TcpStream>>>,
}

impl Forwarder {
  /// Forwards to `target` until taken down.
  pub fn start(target: SocketAddr) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    let port = listener.local_addr().expect("address").port();
    let (up, open) = (Arc::new(AtomicBool::new(true)), Arc::new(Mutex::new(Vec::new())));
    let (accepting, tracked) = (up.clone(), open.clone());
    thread::spawn(move || {
      for client in listener.incoming().flatten() {
        if !accepting.load(Ordering::SeqCst) {
          // Down: the connection ends before anything is read.
          let _ = client.shutdown(Shutdown::Both);
          continue;
        }
        // A connection that cannot be forwarded ends, as when the server is down; the test sees
        // the server unreachable where it expects it connected and fails there.
        let _ = forward(client, target, &tracked);
      }
    });
    Self { port, up, open }
  }

  pub fn url(&self) -> String {
    format!("http://127.0.0.1:{}", self.port)
  }

  /// Takes the port down and ends the connections that are open.
  pub fn down(&self) {
    self.up.store(false, Ordering::SeqCst);
    for stream in self.open.lock().drain(..) {
      // A connection the other side closed already is ended either way.
      let _ = stream.shutdown(Shutdown::Both);
    }
  }

  pub fn up(&self) {
    self.up.store(true, Ordering::SeqCst);
  }
}

/// Copies `client` to the server and back on two threads, keeping both ends to end them later.
fn forward(client: TcpStream, target: SocketAddr, open: &Mutex<Vec<TcpStream>>) -> io::Result<()> {
  let server = TcpStream::connect(target)?;
  open.lock().extend([client.try_clone()?, server.try_clone()?]);
  let (mut client_in, mut server_out) = (client.try_clone()?, server.try_clone()?);
  let (mut server_in, mut client_out) = (server, client);
  thread::spawn(move || {
    // Either side closing ends the copy; that is how a connection finishes.
    let _ = io::copy(&mut client_in, &mut server_out);
    let _ = server_out.shutdown(Shutdown::Write);
  });
  thread::spawn(move || {
    let _ = io::copy(&mut server_in, &mut client_out);
    let _ = client_out.shutdown(Shutdown::Write);
  });
  Ok(())
}
