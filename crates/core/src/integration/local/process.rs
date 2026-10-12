use serde::{Deserialize, Serialize};

use crate::{blob::ContentHash, id::RunId};

/// A process birth identity, without arguments or an executable path that could contain secret values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessIdentity {
  pub pid: u32,
  pub started: u64,
  pub executable: ContentHash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalProcess {
  pub sequence: u64,
  pub identity: Option<ProcessIdentity>,
  pub finished: bool,
}

/// Durable execution lifecycle. A missing birth identity after interruption is an unknown outcome.
pub trait LocalProcessStore {
  type Error: std::error::Error + Send + Sync + 'static;
  fn local_process(&self, run: RunId) -> Result<Option<LocalProcess>, Self::Error>;
  fn begin_local_process(&mut self, run: RunId) -> Result<u64, Self::Error>;
  fn local_process_started(
    &mut self,
    run: RunId,
    sequence: u64,
    identity: Option<&ProcessIdentity>,
  ) -> Result<(), Self::Error>;
  fn finish_local_process(&mut self, run: RunId, sequence: u64) -> Result<(), Self::Error>;
}

/// Execution writes lifecycle records through a storage boundary, without depending on SQLite.
pub trait ProcessJournal: Send + Sync {
  fn begin(&self) -> Result<u64, super::LocalError>;
  fn started(&self, sequence: u64, identity: Option<&ProcessIdentity>) -> Result<(), super::LocalError>;
  fn finished(&self, sequence: u64) -> Result<(), super::LocalError>;
}
