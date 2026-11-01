//! The connection settings selected when the person submits a recorded-request test.
use orchester_core::{
  connection::{DecisionConnection, DecisionServerUrl},
  id::{DecisionConnectionId, SecretId},
};
use serde::{Deserialize, Serialize};

/// Contains only the key reference; the runner resolves its value after checking these settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionTestTarget {
  pub connection: DecisionConnectionId,
  pub base_url: DecisionServerUrl,
  pub key: Option<SecretId>,
}

impl From<&DecisionConnection> for DecisionTestTarget {
  fn from(connection: &DecisionConnection) -> Self {
    Self {
      connection: connection.id(),
      base_url: connection.base_url().clone(),
      key: connection.key(),
    }
  }
}
