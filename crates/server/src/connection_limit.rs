use std::{collections::HashMap, net::IpAddr, sync::Arc};

use parking_lot::Mutex;

use crate::addresses::{TrustedProxy, peer_network};

#[derive(Default)]
struct Counts {
  total: usize,
  peers: HashMap<IpAddr, usize>,
}

struct Limits {
  total: usize,
  per_peer: usize,
  trusted_proxies: Vec<TrustedProxy>,
  counts: Mutex<Counts>,
}

pub(crate) struct ConnectionLimit(Arc<Limits>);

pub(crate) struct ConnectionLease {
  limits: Arc<Limits>,
  peer: Option<IpAddr>,
}

impl ConnectionLimit {
  pub fn new(total: usize, per_peer: usize, trusted_proxies: Vec<TrustedProxy>) -> Self {
    Self(Arc::new(Limits {
      total,
      per_peer,
      trusted_proxies,
      counts: Mutex::default(),
    }))
  }

  pub fn acquire(&self, peer: IpAddr) -> Option<ConnectionLease> {
    // Match the actual address before /64 grouping; trusting one IPv6 proxy must not exempt its neighbours.
    let trusted = self.0.trusted_proxies.iter().any(|proxy| proxy.contains(peer));
    let peer = (!trusted).then(|| peer_network(peer));
    let mut counts = self.0.counts.lock();
    if counts.total >= self.0.total
      || peer.is_some_and(|peer| counts.peers.get(&peer).copied().unwrap_or(0) >= self.0.per_peer)
    {
      return None;
    }
    counts.total += 1;
    if let Some(peer) = peer {
      *counts.peers.entry(peer).or_default() += 1;
    }
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
    let Some(peer) = self.peer else {
      return;
    };
    let count = counts
      .peers
      .get_mut(&peer)
      .expect("a live connection lease retains its peer count");
    *count -= 1;
    if *count == 0 {
      counts.peers.remove(&peer);
    }
  }
}
