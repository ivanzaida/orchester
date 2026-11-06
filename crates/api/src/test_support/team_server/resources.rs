//! A shared project's resources on the stand-in (`GET`/`POST …/resources`, `GET`/`POST …/resources/{r}/versions`,
//! `PATCH …/resources/{r}`; `docs/cloud-api.md`, "Team server reviewer resource" and "Team server MCP server
//! resource"), in the contract's shapes: numbered immutable versions, `409 slug_taken`, `409 version_conflict` with the
//! latest, `409 revision_conflict` with the resource as it is, and `403 not_admin` for a member's write. The body of an
//! MCP server is read as core's `TeamMcpServer`, so the stand-in refuses (`422 invalid`, reason `body`) a body with a
//! path, a value or a local id, and computes `widens` with core's rules; the other kinds' bodies are kept as sent.
//! `GET …/resources/{r}/used-by` answers from the stand-in's own resources and the runs a test pins with
//! [`TeamServer::pin_run`]. A repeated idempotency key returns the first answer. Not served yet:
//! `POST …/resources/import` and the files.

mod used_by;

use std::collections::HashMap;

use orchester_core::{resource::TeamMcpServer, sharing::widens};
use parking_lot::Mutex;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
pub use used_by::PinnedRun;

use super::{Person, Standing, State, TeamServer};
use crate::{ResourceCreate, ResourcePatch, SharedResourceKind, VersionPublish};

/// The resources of every project of the stand-in, by project id.
#[derive(Default)]
pub(super) struct Store {
  projects: HashMap<String, Project>,
}

#[derive(Default)]
struct Project {
  resources: Vec<Stored>,
  /// The first answer of each idempotency key.
  answers: HashMap<String, (u16, Value)>,
  /// The runs [`TeamServer::pin_run`] registered.
  runs: Vec<PinnedRun>,
  /// The number of changes made; the event cursor the list is current at.
  cursor: u64,
}

struct Stored {
  resource: Value,
  versions: Vec<Value>,
}

impl Stored {
  fn id(&self) -> &Value {
    &self.resource["id"]
  }

  fn latest(&self) -> Option<&Value> {
    self.versions.last()
  }
}

impl TeamServer {
  /// The resources of the project at `slug` as the server holds them now, each with its versions, oldest first.
  pub fn resources(&self, slug: &str) -> Vec<(Value, Vec<Value>)> {
    let state = self.state.lock();
    let Some(project) = state.projects.iter().find(|project| project["slug"] == slug) else {
      return Vec::new();
    };
    let id = project["id"].as_str().unwrap_or_default();
    state.resources.projects.get(id).map_or_else(Vec::new, |held| {
      held
        .resources
        .iter()
        .map(|stored| (stored.resource.clone(), stored.versions.clone()))
        .collect()
    })
  }
}

/// The answer to a request under `/api/v1/projects/{p}/resources` of one of `state`'s projects, for the `path` (method
/// first) and `query` and the `body`; `None` for any other request.
pub(super) fn answer((path, query): (&str, &str), body: &str, state: &Mutex<State>) -> Option<(&'static str, Value)> {
  let (verb, rest) = path.split_once(" /api/v1/projects/")?;
  let (project, part) = rest.split_once("/resources")?;
  let mut state = state.lock();
  if !state.projects.iter().any(|known| known["id"] == project) {
    return None;
  }
  let writes = state.authority.standing != Standing::Member;
  let held = state.resources.projects.entry(project.to_owned()).or_default();
  let (status, answer) = match (
    verb,
    part
      .strip_prefix('/')
      .map(|rest| rest.split_once('/').unwrap_or((rest, ""))),
  ) {
    ("GET", None) => (200, json!({"resources": held.list(), "cursor": held.cursor})),
    ("POST" | "PATCH", _) if !writes => (403, error("not_admin")),
    ("POST", None) => held.create(body),
    ("PATCH", Some((resource, ""))) => held.patch(resource, body),
    ("GET", Some((resource, "versions"))) => held.versions(resource, query),
    ("GET", Some((resource, "used-by"))) => held.used_by(resource),
    ("POST", Some((resource, "versions"))) => held.publish(resource, body),
    _ => return None,
  };
  Some((status_line(status), answer))
}

impl Project {
  fn list(&self) -> Vec<&Value> {
    self.resources.iter().map(|stored| &stored.resource).collect()
  }

  fn find(&mut self, resource: &str) -> Option<&mut Stored> {
    self.resources.iter_mut().find(|stored| stored.id() == resource)
  }

  /// Counts one change and says when it happened.
  fn changed(&mut self) -> Value {
    self.cursor += 1;
    json!({"by": Person::You.wire(), "at": i64::try_from(self.cursor).unwrap_or(i64::MAX)})
  }

  fn create(&mut self, body: &str) -> (u16, Value) {
    let Ok(create) = serde_json::from_str::<ResourceCreate>(body) else {
      return invalid("shape");
    };
    let key = create.idempotency_key.to_string();
    if let Some(first) = self.answers.get(&key) {
      return first.clone();
    }
    let kind = serde_json::to_value(create.kind).unwrap_or(Value::Null);
    let id = serde_json::to_value(create.id).unwrap_or(Value::Null);
    let taken = |stored: &&Stored| {
      stored.id() == &id || (stored.resource["kind"] == kind && stored.resource["slug"] == create.slug.as_str())
    };
    if let Some(stored) = self.resources.iter().find(taken) {
      return (
        409,
        json!({"error": "slug_taken", "message": "refused", "resource": stored.resource}),
      );
    }
    let Some(widens) = widened(create.kind, None, &create.version.body) else {
      return invalid("body");
    };
    let attribution = self.changed();
    let version = version(&id, 1, (create.version.body, create.version.references), None);
    let version = with(version, [("published", attribution.clone()), ("widens", json!(widens))]);
    let resource = json!({
      "id": id, "kind": kind, "slug": create.slug, "name": create.name, "archived": false, "revision": 1,
      "latest": 1, "created": attribution, "updated": attribution,
    });
    let answer = (201, json!({"resource": resource, "version": version}));
    self.resources.push(Stored {
      resource,
      versions: vec![version],
    });
    self.answers.insert(key, answer.clone());
    answer
  }

  fn versions(&mut self, resource: &str, query: &str) -> (u16, Value) {
    let after = query
      .strip_prefix("after=")
      .and_then(|after| after.parse::<u64>().ok())
      .unwrap_or(0);
    self.find(resource).map_or_else(
      || (404, error("not_found")),
      |stored| {
        let later = stored
          .versions
          .iter()
          .filter(|version| version["number"].as_u64() > Some(after));
        (200, json!({"versions": later.collect::<Vec<_>>()}))
      },
    )
  }

  fn publish(&mut self, resource: &str, body: &str) -> (u16, Value) {
    let Ok(publish) = serde_json::from_str::<VersionPublish>(body) else {
      return invalid("shape");
    };
    let key = publish.idempotency_key.to_string();
    if let Some(first) = self.answers.get(&key) {
      return first.clone();
    }
    let attribution = self.changed();
    let Some(stored) = self.find(resource) else {
      return (404, error("not_found"));
    };
    if stored.resource["archived"] == true {
      return (409, error("archived"));
    }
    let latest = stored.latest().cloned().unwrap_or(Value::Null);
    let number = latest["number"].as_u64().unwrap_or(0);
    if publish.based_on.map(|based| u64::from(based.get())) != Some(number) {
      return (
        409,
        json!({"error": "version_conflict", "message": "refused", "latest": latest}),
      );
    }
    let kind = serde_json::from_value::<SharedResourceKind>(stored.resource["kind"].clone());
    let Some(widens) = kind
      .ok()
      .and_then(|kind| widened(kind, Some(&latest["body"]), &publish.body))
    else {
      return invalid("body");
    };
    let published = version(
      &stored.resource["id"],
      number + 1,
      (publish.body, publish.references),
      publish.based_on.map(|based| u64::from(based.get())),
    );
    let published = with(
      published,
      [("published", attribution.clone()), ("widens", json!(widens))],
    );
    stored.resource["latest"] = json!(number + 1);
    stored.resource["updated"] = attribution;
    stored.versions.push(published.clone());
    let answer = (201, json!({"version": published}));
    self.answers.insert(key, answer.clone());
    answer
  }

  fn patch(&mut self, resource: &str, body: &str) -> (u16, Value) {
    let Ok(patch) = serde_json::from_str::<ResourcePatch>(body) else {
      return invalid("shape");
    };
    let key = patch.idempotency_key.to_string();
    if let Some(first) = self.answers.get(&key) {
      return first.clone();
    }
    let attribution = self.changed();
    let Some(stored) = self.find(resource) else {
      return (404, error("not_found"));
    };
    if stored.resource["revision"].as_u64() != Some(patch.revision.get()) {
      return (
        409,
        json!({"error": "revision_conflict", "message": "refused", "current": stored.resource}),
      );
    }
    if let Some(name) = patch.name {
      stored.resource["name"] = json!(name);
    }
    if let Some(archived) = patch.archived {
      stored.resource["archived"] = json!(archived);
    }
    stored.resource["revision"] = json!(patch.revision.get() + 1);
    stored.resource["updated"] = attribution;
    let answer = (200, json!({"resource": stored.resource}));
    self.answers.insert(key, answer.clone());
    answer
  }
}

/// Whether `body`, a version of a `kind` resource after `previous`, widens what an agent may do; `None` when the body
/// is not one of the kind.
fn widened(kind: SharedResourceKind, previous: Option<&Value>, body: &Value) -> Option<bool> {
  match kind {
    SharedResourceKind::McpServer => {
      let next = serde_json::from_value::<TeamMcpServer>(body.clone()).ok()?;
      let before = previous.and_then(|previous| serde_json::from_value::<TeamMcpServer>(previous.clone()).ok());
      Some(!widens(before.as_ref(), &next).is_empty())
    }
    _ => Some(false),
  }
}

/// A version before its attribution and `widens`: the stand-in's hash of the body is a SHA-256 of its JSON.
fn version(
  resource: &Value,
  number: u64,
  (body, references): (Value, Vec<crate::Reference>),
  based_on: Option<u64>,
) -> Value {
  let hash: String = Sha256::digest(body.to_string().as_bytes())
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect();
  json!({
    "resource": resource, "number": number, "body": body, "hash": hash, "references": references,
    "based_on": based_on,
  })
}

/// `object` with `fields` set.
fn with<const N: usize>(mut object: Value, fields: [(&str, Value); N]) -> Value {
  for (name, value) in fields {
    object[name] = value;
  }
  object
}

fn invalid(reason: &str) -> (u16, Value) {
  (422, json!({"error": "invalid", "message": "refused", "reason": reason}))
}

fn error(code: &str) -> Value {
  json!({"error": code, "message": "refused"})
}

fn status_line(status: u16) -> &'static str {
  match status {
    200 => "200 OK",
    201 => "201 Created",
    403 => "403 Forbidden",
    404 => "404 Not Found",
    409 => "409 Conflict",
    _ => "422 Unprocessable Entity",
  }
}
