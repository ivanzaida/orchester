//! The fake team server's writes of the team's configuration, by its admins only (`403 not_admin` otherwise): a
//! resource created with its v1, a version published on the latest one (`409 version_conflict` with the latest
//! otherwise, `422 missing_files` for a skill part it does not have), and a skill part uploaded by its hash. Each
//! version appends its `resource.published`.

use std::{net::TcpStream, num::NonZeroU32};

use orchester_api::{
  Attribution, Event, Reference, ResourceCreate, ResourcePublished, SharedResource, SharedResourceKind, SharedVersion,
  UserRef, VersionPublish,
};
use orchester_core::{blob::ContentHash, id::ResourceId, resource::VersionNumber, sharing::TaskRevision};
use serde_json::{Value, json};

use super::{Shared, State};
use crate::http::{Request, refusal, respond};

impl State {
  /// `by` publishes the next version of resource `id`, a `kind` named `name` that is created with its v1: `body`,
  /// naming `references`. Appends its `resource.published`.
  pub(super) fn publish_version(
    &mut self,
    by: &UserRef,
    (id, kind, name): (ResourceId, SharedResourceKind, &str),
    body: Value,
    references: Vec<Reference>,
  ) -> SharedVersion {
    let published = Attribution {
      by: by.clone(),
      at: self.tick(),
    };
    let based_on = self.latest(id);
    let number = VersionNumber::new(
      NonZeroU32::new(based_on.map_or(0, VersionNumber::get) + 1).expect("a version number above zero"),
    );
    let version = SharedVersion {
      resource: id,
      number,
      body,
      hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
      references,
      based_on,
      published: published.clone(),
    };
    self.versions.push(version.clone());
    let resource = if let Some(resource) = self.resources.iter_mut().find(|resource| resource.id == id) {
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
      self.resources.push(resource.clone());
      resource
    };
    self.append(by, ResourcePublished { resource }, Event::ResourcePublished);
    version
  }

  fn latest(&self, id: ResourceId) -> Option<VersionNumber> {
    self
      .resources
      .iter()
      .find(|resource| resource.id == id)
      .and_then(|resource| resource.latest)
  }

  /// The skill parts `body` names that the project does not have.
  fn missing_files(&self, body: &Value) -> Vec<String> {
    let named = body["files"].as_array().map_or(&[][..], Vec::as_slice);
    named
      .iter()
      .filter_map(|file| file["content"].as_str())
      .filter(|hash| !self.files.iter().any(|(kept, _)| kept.as_str() == *hash))
      .map(str::to_owned)
      .collect()
  }
}

/// `POST …/resources`: the resource with its v1.
pub(super) fn create(shared: &Shared, user: &UserRef, request: &Request, stream: &mut TcpStream) {
  let create: ResourceCreate = request.json();
  let mut state = shared.state.lock().expect("state");
  let missing = state.missing_files(&create.version.body);
  if !missing.is_empty() {
    drop(state);
    return missing_files(stream, &missing);
  }
  let version = state.publish_version(
    user,
    (create.id, create.kind, &create.name),
    create.version.body,
    create.version.references,
  );
  let resource = state
    .resources
    .iter()
    .find(|resource| resource.id == create.id)
    .cloned()
    .expect("created");
  drop(state);
  shared.appended.notify_all();
  respond(stream, 201, &json!({ "resource": resource, "version": version }));
}

/// `POST …/resources/{resource}/versions`: the next version, on the latest one.
pub(super) fn publish(shared: &Shared, user: &UserRef, resource: &str, request: &Request, stream: &mut TcpStream) {
  let publish: VersionPublish = request.json();
  let id: ResourceId = resource.parse().expect("a resource id");
  let mut state = shared.state.lock().expect("state");
  let latest = state.latest(id);
  if publish.based_on != latest {
    let latest = state
      .versions
      .iter()
      .filter(|version| version.resource == id)
      .max_by_key(|version| version.number)
      .cloned();
    drop(state);
    let answer = json!({ "error": "version_conflict", "message": "Refused", "latest": latest });
    return respond(stream, 409, &answer);
  }
  let missing = state.missing_files(&publish.body);
  if !missing.is_empty() {
    drop(state);
    return missing_files(stream, &missing);
  }
  let (kind, name) = {
    let resource = state
      .resources
      .iter()
      .find(|resource| resource.id == id)
      .expect("a resource of the project");
    (resource.kind, resource.name.clone())
  };
  let version = state.publish_version(user, (id, kind, &name), publish.body, publish.references);
  drop(state);
  shared.appended.notify_all();
  respond(stream, 201, &json!({ "version": version }));
}

/// `PUT …/files/{sha256}`: `201` for a part the project did not have, `200` for one it had.
pub(super) fn upload(shared: &Shared, hash: &str, request: &Request, stream: &mut TcpStream) {
  let hash = ContentHash::new(hash).expect("a hash");
  let mut state = shared.state.lock().expect("state");
  if state.files.iter().any(|(kept, _)| *kept == hash) {
    drop(state);
    return respond(stream, 200, &json!({}));
  }
  state.files.push((hash, request.bytes().to_vec()));
  drop(state);
  respond(stream, 201, &json!({}));
}

fn missing_files(stream: &mut TcpStream, hashes: &[String]) {
  let answer = json!({ "error": "missing_files", "message": "Refused", "hashes": hashes });
  respond(stream, 422, &answer);
}

/// A write of the configuration by someone who is not an admin of the project.
pub(super) fn not_admin(stream: &mut TcpStream) {
  refusal(stream, 403, "not_admin");
}
