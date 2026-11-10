//! One team project's configuration as the team server has it: each resource with its latest version only
//! (`docs/design/team-configuration-live.md`). An older version is never read, so one the server cannot rewrite into
//! the team's form costs nothing here; a latest version whose body is not its kind's team form is kept as it came, and
//! its readers say it cannot be told yet instead of dropping the resource.

use std::collections::BTreeMap;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{ResourceKind, VersionNumber},
  sharing::TaskRevision,
  time::Timestamp,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

/// The team's resources of one project, archived ones too, by id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TeamConfig {
  resources: BTreeMap<ResourceId, TeamResource>,
}

impl TeamConfig {
  pub fn new(resources: impl IntoIterator<Item = TeamResource>) -> Self {
    Self {
      resources: resources.into_iter().map(|resource| (resource.id, resource)).collect(),
    }
  }

  /// The resources of `kind` the team has not archived, by name.
  pub fn resources(&self, kind: ResourceKind) -> Vec<&TeamResource> {
    let mut found: Vec<&TeamResource> = self
      .resources
      .values()
      .filter(|resource| resource.kind == kind && !resource.archived)
      .collect();
    found.sort_by_cached_key(|resource| resource.name.as_str().to_lowercase());
    found
  }

  /// The resources of `kind` the team archived.
  pub fn archived_of(&self, kind: ResourceKind) -> impl Iterator<Item = &TeamResource> {
    self
      .resources
      .values()
      .filter(move |resource| resource.kind == kind && resource.archived)
  }

  /// The resource `id`, archived or not.
  pub fn resource(&self, id: ResourceId) -> Option<&TeamResource> {
    self.resources.get(&id)
  }

  /// The resource `id` while the team has not archived it.
  pub fn live(&self, id: ResourceId) -> Option<&TeamResource> {
    self.resource(id).filter(|resource| !resource.archived)
  }

  /// The archived resource `id` with its name; `None` while it is not archived or not known.
  pub fn archived(&self, id: ResourceId) -> Option<&TeamResource> {
    self.resource(id).filter(|resource| resource.archived)
  }

  /// The name of `id`, archived or not; `None` for a resource this configuration does not hold.
  pub fn name(&self, id: ResourceId) -> Option<&str> {
    self.resource(id).map(|resource| resource.name.as_str())
  }

  /// Keeps `resource` in place of what this configuration held of it.
  pub(crate) fn keep(&mut self, resource: TeamResource) {
    self.resources.insert(resource.id, resource);
  }

  /// Follows the team's archived mark of `id`; whether it changed.
  pub(crate) fn mark_archived(&mut self, id: ResourceId, archived: bool) -> bool {
    match self.resources.get_mut(&id) {
      Some(resource) if resource.archived != archived => {
        resource.archived = archived;
        true
      }
      _ => false,
    }
  }

  /// Renames `id`; whether it changed.
  pub(crate) fn rename(&mut self, id: ResourceId, name: Name) -> bool {
    match self.resources.get_mut(&id) {
      Some(resource) if resource.name != name => {
        resource.name = name;
        true
      }
      _ => false,
    }
  }
}

/// A resource of the team with its latest version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamResource {
  pub id: ResourceId,
  pub kind: ResourceKind,
  pub name: Name,
  pub archived: bool,
  /// The resource's revision on the server, which a rename or an archive is made on.
  pub revision: TaskRevision,
  /// `None` while the server has numbered no version of it.
  pub latest: Option<TeamVersion>,
}

impl TeamResource {
  /// The latest version's body read as `B`, its kind's team form: `None` without a version, `Some(Err)` for a body that
  /// is not in that form (e.g. one published before the team's form), which is logged.
  pub fn body<B: DeserializeOwned>(&self) -> Option<Result<B, NotTeamForm>> {
    let latest = self.latest.as_ref()?;
    Some(serde_json::from_value::<B>(latest.body.clone()).map_err(|error| {
      tracing::warn!(resource = %self.id, number = latest.number.get(), %error, "the team's latest version is not in its kind's team form");
      NotTeamForm
    }))
  }

  /// The latest version's number; `None` while there is none.
  pub fn number(&self) -> Option<VersionNumber> {
    self.latest.as_ref().map(|latest| latest.number)
  }
}

/// A latest version whose body is not in its kind's team form, so what it asks of this computer cannot be told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotTeamForm;

/// The latest version of a team resource as the server sent it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamVersion {
  pub number: VersionNumber,
  pub body: Value,
  /// Who published it, by the name the server gives them.
  pub published_by: String,
  pub published_at: Timestamp,
}
