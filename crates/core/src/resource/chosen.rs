//! What a local agent profile gives its agents besides its MCP servers: the project's instructions, skills or tools it
//! chooses (`docs/design/team-configuration.md`, "Decided 2026-11-09"). Nothing else gives an agent one: an
//! instruction's scope and targets, a skill's scope and a tool's callers decide nothing for a local run.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::id::ResourceId;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ChosenResourcesError {
  #[error("a profile chooses {0} twice")]
  Duplicate(ResourceId),
}

/// Resources of one kind a profile chooses, each once, in the order chosen. There is no upper bound: a profile keeps
/// everything that reached its agents before profiles chose them, however many that was.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<ResourceId>", into = "Vec<ResourceId>")]
pub struct ChosenResources(Vec<ResourceId>);

impl ChosenResources {
  pub const EMPTY: Self = Self(Vec::new());

  pub fn new(resources: Vec<ResourceId>) -> Result<Self, ChosenResourcesError> {
    if let Some(repeated) = resources
      .iter()
      .enumerate()
      .find_map(|(at, resource)| resources[..at].contains(resource).then_some(*resource))
    {
      return Err(ChosenResourcesError::Duplicate(repeated));
    }
    Ok(Self(resources))
  }

  pub fn ids(&self) -> &[ResourceId] {
    &self.0
  }

  pub fn contains(&self, resource: ResourceId) -> bool {
    self.0.contains(&resource)
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  /// These and `resource` after them; the same when it is chosen already.
  #[must_use]
  pub fn with(&self, resource: ResourceId) -> Self {
    let mut resources = self.0.clone();
    if !resources.contains(&resource) {
      resources.push(resource);
    }
    Self(resources)
  }

  /// These without `resource`.
  #[must_use]
  pub fn without(&self, resource: ResourceId) -> Self {
    Self(self.0.iter().copied().filter(|chosen| *chosen != resource).collect())
  }
}

impl TryFrom<Vec<ResourceId>> for ChosenResources {
  type Error = ChosenResourcesError;

  fn try_from(resources: Vec<ResourceId>) -> Result<Self, ChosenResourcesError> {
    Self::new(resources)
  }
}

impl From<ChosenResources> for Vec<ResourceId> {
  fn from(chosen: ChosenResources) -> Self {
    chosen.0
  }
}
