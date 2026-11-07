//! Why a project's readiness could not be read.

use orchester_store::StoreError;
use thiserror::Error;

use crate::messages::{self, Message};

#[derive(Debug, Error)]
pub enum ReadinessError {
  #[error(transparent)]
  Store(#[from] StoreError),
}

impl ReadinessError {
  /// What to tell the person.
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
    }
  }
}
