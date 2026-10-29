//! The fake team server's resources: a version published appends its `resource.published` ([`super::publishing`]),
//! and a resource's versions are read one a page.

use orchester_api::{Reference, SharedResourceKind, SharedVersion, Versions};
use orchester_core::{blob::ContentHash, id::ResourceId, resource::VersionNumber};
use serde_json::Value;

use super::{State, TeamServer};

impl TeamServer {
  /// The first member publishes the next version of resource `id`, a `kind` named `name` that is created with its v1:
  /// `body`, naming `references`. Appends its `resource.published`.
  pub fn publish(
    &self,
    resource: (ResourceId, SharedResourceKind, &str),
    body: Value,
    references: Vec<Reference>,
  ) -> VersionNumber {
    let mut state = self.state();
    let by = state.members[0].1.clone();
    let version = state.publish_version(&by, resource, body, references);
    drop(state);
    self.shared.appended.notify_all();
    version.number
  }

  /// `token`'s member changes the team's configuration from now on, as the server's admin does.
  pub fn make_admin(&self, token: &str) {
    self.state().admins.push(token.to_owned());
  }

  /// The versions of `resource` the server has, oldest first.
  pub fn versions_of(&self, resource: ResourceId) -> Vec<SharedVersion> {
    let state = self.state();
    state
      .versions
      .iter()
      .filter(|version| version.resource == resource)
      .cloned()
      .collect()
  }

  /// The content of the skill part `hash`, when it was uploaded.
  pub fn file(&self, hash: &ContentHash) -> Option<Vec<u8>> {
    let state = self.state();
    state
      .files
      .iter()
      .find(|(kept, _)| kept == hash)
      .map(|(_, content)| content.clone())
  }
}

impl State {
  /// `GET …/resources/{resource}/versions?after=`: one version a page, so a resource with several takes several pages.
  pub(super) fn versions_page(&self, resource: &str, query: &str) -> Value {
    let id: ResourceId = resource.parse().expect("a resource id");
    let after: u32 = query
      .strip_prefix("after=")
      .map_or(0, |after| after.parse().expect("a version number"));
    let mut newer = self
      .versions
      .iter()
      .filter(|version| version.resource == id && version.number.get() > after);
    let versions: Vec<SharedVersion> = newer.next().cloned().into_iter().collect();
    let next = newer.next().and(versions.last()).map(|version| version.number);
    serde_json::to_value(Versions { versions, next }).expect("versions serialize")
  }
}
