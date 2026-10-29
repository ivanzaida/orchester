//! The fake team server's resources: a version published appends its `resource.published`, and a resource's versions
//! are read one a page.

use std::num::NonZeroU32;

use orchester_api::{
  Attribution, Event, Reference, ResourcePublished, SharedResource, SharedResourceKind, SharedVersion, Versions,
};
use orchester_core::{blob::ContentHash, id::ResourceId, resource::VersionNumber, sharing::TaskRevision};
use serde_json::Value;

use super::{State, TeamServer};

impl TeamServer {
  /// The first member publishes the next version of resource `id`, a `kind` named `name` that is created with its v1:
  /// `body`, naming `references`. Appends its `resource.published`.
  pub fn publish(
    &self,
    (id, kind, name): (ResourceId, SharedResourceKind, &str),
    body: Value,
    references: Vec<Reference>,
  ) -> VersionNumber {
    let mut state = self.state();
    let by = state.members[0].1.clone();
    let published = Attribution {
      by: by.clone(),
      at: state.tick(),
    };
    let based_on = state
      .resources
      .iter()
      .find(|resource| resource.id == id)
      .and_then(|resource| resource.latest);
    let number = VersionNumber::new(
      NonZeroU32::new(based_on.map_or(0, VersionNumber::get) + 1).expect("a version number above zero"),
    );
    state.versions.push(SharedVersion {
      resource: id,
      number,
      body,
      hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
      references,
      based_on,
      published: published.clone(),
    });
    let resource = if let Some(resource) = state.resources.iter_mut().find(|resource| resource.id == id) {
      resource.latest = Some(number);
      resource.updated = published;
      resource.clone()
    } else {
      let resource = SharedResource {
        id,
        kind,
        slug: name.to_lowercase().replace(' ', "-"),
        name: name.to_owned(),
        archived: false,
        revision: TaskRevision::FIRST,
        latest: Some(number),
        created: published.clone(),
        updated: published,
      };
      state.resources.push(resource.clone());
      resource
    };
    state.append(&by, ResourcePublished { resource }, Event::ResourcePublished);
    drop(state);
    self.shared.appended.notify_all();
    number
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
