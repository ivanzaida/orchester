use std::{
  net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
  str::FromStr,
};

use axum::http::HeaderMap;
use serde::{Deserialize, Deserializer};

use crate::error::ServerError;

/// A trusted proxy's exact address or CIDR network, with mapped IPv4 addresses canonicalized.
#[derive(Clone, Debug)]
pub struct TrustedProxy {
  network: IpAddr,
  prefix: u8,
}

impl FromStr for TrustedProxy {
  type Err = ServerError;

  fn from_str(value: &str) -> Result<Self, Self::Err> {
    let invalid = || ServerError::Configuration("invalid trusted proxy address or CIDR");
    let (address, prefix) = value
      .split_once('/')
      .map_or((value, None), |(ip, bits)| (ip, Some(bits)));
    let address: IpAddr = address.parse().map_err(|_| invalid())?;
    let bits = if address.is_ipv4() { 32 } else { 128 };
    let prefix = prefix.map_or(Ok(bits), str::parse::<u8>).map_err(|_| invalid())?;
    if prefix > bits {
      return Err(invalid());
    }
    let canonical = address.to_canonical();
    let prefix = if address.is_ipv6() && canonical.is_ipv4() {
      prefix.checked_sub(96).ok_or_else(invalid)?
    } else {
      prefix
    };
    Ok(Self {
      network: masked(canonical, prefix),
      prefix,
    })
  }
}

impl<'de> Deserialize<'de> for TrustedProxy {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    String::deserialize(deserializer)?
      .parse()
      .map_err(serde::de::Error::custom)
  }
}

impl TrustedProxy {
  fn contains(&self, address: IpAddr) -> bool {
    let address = address.to_canonical();
    address.is_ipv4() == self.network.is_ipv4() && masked(address, self.prefix) == self.network
  }
}

fn masked(address: IpAddr, prefix: u8) -> IpAddr {
  match address {
    IpAddr::V4(ip) => {
      let mask = u32::MAX.checked_shl(u32::from(32 - prefix)).unwrap_or(0);
      Ipv4Addr::from(u32::from(ip) & mask).into()
    }
    IpAddr::V6(ip) => {
      let mask = u128::MAX.checked_shl(u32::from(128 - prefix)).unwrap_or(0);
      Ipv6Addr::from(u128::from(ip) & mask).into()
    }
  }
}

fn hop(value: &str) -> Option<IpAddr> {
  let value = value.trim();
  if let Ok(ip) = value.parse::<IpAddr>() {
    return Some(ip.to_canonical());
  }
  if let Ok(socket) = value.parse::<SocketAddr>() {
    return Some(socket.ip().to_canonical());
  }
  let ip = value.strip_prefix('[')?.strip_suffix(']')?.parse::<Ipv6Addr>();
  // A malformed bracketed address intentionally falls back to the connection peer.
  ip.ok().map(|ip| IpAddr::V6(ip).to_canonical())
}

pub(crate) fn client_address(peer: IpAddr, proxies: &[TrustedProxy], headers: &HeaderMap) -> IpAddr {
  let peer = peer.to_canonical();
  let trusted = |address| proxies.iter().any(|proxy| proxy.contains(address));
  if !trusted(peer) {
    return peer;
  }
  let mut hops = Vec::new();
  // Multiple field lines form one ordered comma list (RFC 9110); validate the entire list first.
  for header in headers.get_all("X-Forwarded-For") {
    let Ok(header) = header.to_str() else {
      return peer;
    };
    for value in header.split(',') {
      let Some(ip) = hop(value) else {
        return peer;
      };
      hops.push(ip);
    }
  }
  let mut address = peer;
  for hop in hops.into_iter().rev() {
    if !trusted(address) {
      break;
    }
    address = hop;
  }
  address
}

pub(crate) fn rate_address(address: IpAddr) -> IpAddr {
  let address = address.to_canonical();
  if address.is_ipv6() {
    masked(address, 64)
  } else {
    address
  }
}
