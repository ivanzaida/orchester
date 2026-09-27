//! Import resources (06.04 `W4zGKx`, 06.05 `QqnDd`, 06.06 `hF664`): copying pipelines, agent
//! profiles, instructions and tools from another project of the user into this one.
//!
//! A copy is a new resource of this project, with its own id, whose history starts as a draft:
//! nothing is published and no run starts. Later edits in the source project do not reach it;
//! where it came from (project, resource, version) is recorded for display only.
//!
//! Whatever a copy refers to is resolved in this project ([`Plan`]): a resource is copied too or
//! replaced by one of this project's. A connection or a secret is never copied — access does not
//! move between projects — so it is mapped to one of this project's or left unresolved; a copy
//! then keeps naming one this project does not have, a problem its draft shows until it changes.

mod batch;
mod catalog;
mod plan;
mod references;

pub use self::{
  batch::{CopiedResource, ImportBatch, ImportError, Provenance},
  catalog::{Catalog, CatalogResource, Content, Pick},
  plan::{Choice, Choices, Counts, Item, Plan, Problem, Resolution, Role},
};
use crate::id::{ConnectionId, ResourceId, SecretId};

/// A resource, connection or secret of a project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Entity {
  Resource(ResourceId),
  Connection(ConnectionId),
  Secret(SecretId),
}

impl Entity {
  const fn uuid(self) -> uuid::Uuid {
    match self {
      Self::Resource(id) => id.as_uuid(),
      Self::Connection(id) => id.as_uuid(),
      Self::Secret(id) => id.as_uuid(),
    }
  }
}
