//! The coordinator link (`docs/architecture/remote-runners.md`, "Rules of the boundary", rule 1): the narrow
//! interface through which the executor asks for the project's data instead of opening the project database. The
//! local implementation, the coordinator's, reads the database as before; a remote one answers the same requests
//! from another computer later.
//!
//! Every request and answer is a plain value that serializes (serde): never a store connection, a row borrowed from
//! one or a closure. Reads keep their timing: what an agent start read before, it asks the link at each start.
//!
//! Each area of the project's data has a trait and a module of its own, which [`CoordinatorLink`] gathers, so an area
//! added later (approvals, settings) adds its trait and module without touching another's:
//! - [`resources`]: the resource versions a run pinned, read by id, and the blobs they refer to.
//! - [`messages`]: the task's conversation, and the files people attached to its messages with their content.
//! - [`environment`]: the variables Project settings › Environment sets, which the computer's resources turn into the
//!   environment there.
//! - [`memory`]: the task and project memory a prompt gets, the records that cannot be read back, the Memory setting.
//!
//! No secret value is part of any message: the executor reads values on its own computer (`crate::computer`).

pub mod environment;
pub mod memory;
pub mod messages;
pub mod resources;
#[cfg(feature = "test-agent")]
pub mod sent;

use orchester_core::run::RunnerFailure;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::{environment::EnvironmentLink, memory::MemoryLink, messages::MessageLink, resources::ResourceLink};

/// Everything the executor asks its coordinator: one trait per area.
pub trait CoordinatorLink: ResourceLink + MessageLink + EnvironmentLink + MemoryLink {
  /// Another link to the same coordinator, for a task of the run's own: an agent's worker, which reads the messages
  /// sent to its agent while its turn runs and the project's variables its reviews read, and an agent's tool calls,
  /// which read the run's records.
  fn another(&self) -> Result<Box<dyn CoordinatorLink + Send>, LinkError>;
}

/// Why the coordinator could not answer a request: what reading failed with, as text, and what kind of failure it is
/// when it stops the run ([`RunnerFailure`]: the database stayed busy, is unavailable, or something else).
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
#[error("{detail}")]
pub struct LinkError {
  pub detail: String,
  pub failure: RunnerFailure,
}

impl LinkError {
  /// A failure that is not the project database's, described by `error`.
  pub fn other(error: &dyn std::error::Error) -> Self {
    Self {
      detail: error.to_string(),
      failure: RunnerFailure::Other,
    }
  }
}
