//! The loopback ports the runner's services hold. The system hands out a free one; a port another service of the
//! runner was given and still holds is passed over, so two runs never get the same port, even before the first one's
//! program listens on it.

use std::{
  collections::BTreeSet,
  io,
  net::{Ipv4Addr, TcpListener},
  sync::Arc,
};

use parking_lot::Mutex;

/// How many ports the system may hand out that another service holds before a start gives up.
const TRIES: usize = 64;

/// The ports given to services that have not stopped yet.
#[derive(Default)]
pub(super) struct Ports(Mutex<BTreeSet<u16>>);

impl Ports {
  /// A free loopback port no service of the runner holds, held until the lease is dropped.
  pub(super) fn lease(self: &Arc<Self>) -> io::Result<PortLease> {
    let mut held = self.0.lock();
    for _ in 0..TRIES {
      // The listener is closed at once: the service's program binds the port itself.
      let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?.local_addr()?.port();
      if held.insert(port) {
        return Ok(PortLease {
          port,
          ports: Arc::clone(self),
        });
      }
    }
    Err(io::Error::other(
      "the system kept handing out ports other services of this runner hold",
    ))
  }
}

/// A port a service holds; dropping it frees the port for others.
pub(super) struct PortLease {
  port: u16,
  ports: Arc<Ports>,
}

impl PortLease {
  pub(super) const fn port(&self) -> u16 {
    self.port
  }
}

impl Drop for PortLease {
  fn drop(&mut self) {
    self.ports.0.lock().remove(&self.port);
  }
}
