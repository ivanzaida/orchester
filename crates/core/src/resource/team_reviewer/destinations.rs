use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
  connection::{DecisionServerUrl, is_this_computer},
  permission::{HostPattern, HostPatternError},
};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ReviewerHostError {
  #[error("a reviewer host names one server, without a wildcard")]
  Wildcard,
  #[error(transparent)]
  Host(#[from] HostPatternError),
}

/// One exact DNS name or IP address, without a scheme, port or path; names are case insensitive.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ReviewerHost(String);

impl ReviewerHost {
  pub fn new(value: &str) -> Result<Self, ReviewerHostError> {
    let value = value.trim();
    let unbracketed = value
      .strip_prefix('[')
      .and_then(|v| v.strip_suffix(']'))
      .unwrap_or(value);
    if let Ok(address) = unbracketed.parse::<IpAddr>() {
      return Ok(Self(address.to_string()));
    }
    let host = HostPattern::new(value)?;
    if host.as_str().starts_with("*.") {
      return Err(ReviewerHostError::Wildcard);
    }
    Ok(Self(host.as_str().to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl TryFrom<String> for ReviewerHost {
  type Error = ReviewerHostError;

  fn try_from(value: String) -> Result<Self, Self::Error> {
    Self::new(&value)
  }
}

impl From<ReviewerHost> for String {
  fn from(value: ReviewerHost) -> Self {
    value.0
  }
}

/// Local servers are always allowed; a hosts list additionally permits only those exact remote hosts.
/// An empty hosts list permits no remote server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "hosts", rename_all = "snake_case")]
pub enum ReviewerDestinations {
  MembersComputers,
  Hosts(std::collections::BTreeSet<ReviewerHost>),
}

impl ReviewerDestinations {
  pub fn allows_server(&self, server: &DecisionServerUrl) -> bool {
    if server.is_on_this_computer() {
      return true;
    }
    let Self::Hosts(hosts) = self else {
      return false;
    };
    ReviewerHost::new(server.host()).is_ok_and(|host| hosts.contains(&host))
  }

  pub(super) fn adds_to(&self, previous: &Self) -> bool {
    let Self::Hosts(hosts) = self else {
      return false;
    };
    hosts.iter().any(|host| {
      if is_this_computer(host.as_str()) {
        return false;
      }
      match previous {
        Self::MembersComputers => true,
        Self::Hosts(before) => !before.contains(host),
      }
    })
  }
}
