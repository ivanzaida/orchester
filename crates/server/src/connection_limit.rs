use std::{collections::HashMap, net::IpAddr, sync::Arc};

use parking_lot::Mutex;

use crate::addresses::rate_address;

#[derive(Default)]
struct Counts {
  total: usize,
  peers: HashMap<IpAddr, usize>,
}

struct Limits {
  total: usize,
  per_peer: usize,
  counts: Mutex<Counts>,
}

pub(crate) struct ConnectionLimit(Arc<Limits>);

pub(crate) struct ConnectionLease {
  limits: Arc<Limits>,
  peer: IpAddr,
}

impl ConnectionLimit {
  pub fn new(total: usize, per_peer: usize) -> Self {
    Self(Arc::new(Limits {
      total,
      per_peer,
      counts: Mutex::default(),
    }))
  }

  pub fn acquire(&self, peer: IpAddr) -> Option<ConnectionLease> {
    let peer = rate_address(peer);
    let mut counts = self.0.counts.lock();
    if counts.total >= self.0.total || counts.peers.get(&peer).copied().unwrap_or(0) >= self.0.per_peer {
      return None;
    }
    counts.total += 1;
    *counts.peers.entry(peer).or_default() += 1;
    Some(ConnectionLease {
      limits: self.0.clone(),
      peer,
    })
  }
}

impl Drop for ConnectionLease {
  fn drop(&mut self) {
    let mut counts = self.limits.counts.lock();
    counts.total -= 1;
    let count = counts
      .peers
      .get_mut(&self.peer)
      .expect("a live connection lease retains its peer count");
    *count -= 1;
    if *count == 0 {
      counts.peers.remove(&self.peer);
    }
  }
}
