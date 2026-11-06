//! `GET …/resources/{r}/used-by` on the stand-in: the other resources that are not archived and whose latest version
//! names the resource (its id appears in the body or the references), by name and then id, and the runs registered
//! with [`TeamServer::pin_run`] that have not ended and pin a version of it, by task key and then run number.

use orchester_core::{
  id::{ResourceId, RunId},
  resource::VersionNumber,
};
use serde_json::{Value, json};

use super::{Project, Stored, error};
use crate::test_support::team_server::{Person, TeamServer};

/// A run of a task of the project at some slug that pins versions of shared resources: the pipeline, a step's profile
/// or tool, an instruction.
#[derive(Clone, Debug)]
pub struct PinnedRun {
  /// The key of the run's task, e.g. `KON-12`.
  pub task_key: String,
  pub run: RunId,
  pub number: u32,
  /// Who holds the task's claim.
  pub holder: Person,
  /// The run has ended: it is not in the answer.
  pub ended: bool,
  /// The versions it pins.
  pub pins: Vec<(ResourceId, VersionNumber)>,
}

impl TeamServer {
  /// The project at `slug` has `run`, which counts in used-by while it has not ended and pins a version of the
  /// resource asked about. Panics for a slug the stand-in does not hold, so a test cannot pin to nothing.
  pub fn pin_run(&self, slug: &str, run: PinnedRun) {
    let mut state = self.state.lock();
    let id = state
      .projects
      .iter()
      .find(|project| project["slug"] == slug)
      .and_then(|project| project["id"].as_str())
      .expect("`pin_run` names a project the stand-in holds")
      .to_owned();
    state.resources.projects.entry(id).or_default().runs.push(run);
  }
}

impl Project {
  pub(super) fn used_by(&self, resource: &str) -> (u16, Value) {
    if !self.resources.iter().any(|stored| stored.id() == resource) {
      return (404, error("not_found"));
    }
    let mut resources: Vec<&Stored> = self
      .resources
      .iter()
      .filter(|stored| stored.id() != resource && stored.resource["archived"] == false)
      .filter(|stored| stored.latest().is_some_and(|latest| names(latest, resource)))
      .collect();
    resources.sort_by(|a, b| {
      (a.resource["name"].as_str(), a.id().as_str()).cmp(&(b.resource["name"].as_str(), b.id().as_str()))
    });
    let mut runs: Vec<&PinnedRun> = self
      .runs
      .iter()
      .filter(|run| !run.ended && run.pins.iter().any(|(pinned, _)| pinned.to_string() == resource))
      .collect();
    runs.sort_by(|a, b| (&a.task_key, a.number).cmp(&(&b.task_key, b.number)));
    (
      200,
      json!({
        "resources": resources.iter().map(|stored| json!({
          "kind": stored.resource["kind"], "id": stored.resource["id"], "name": stored.resource["name"],
          "version": stored.resource["latest"],
        })).collect::<Vec<_>>(),
        "runs": runs.iter().map(|run| json!({
          "task_key": run.task_key, "run": run.run, "number": run.number, "holder": run.holder.wire(),
        })).collect::<Vec<_>>(),
      }),
    )
  }
}

/// Whether `version`'s body or references hold `id` as a string, wherever it sits, so a change of the reference shape
/// (a team resource named by kind and server id) still counts. Server ids only: no local id is read.
fn names(version: &Value, id: &str) -> bool {
  contains(&version["body"], id) || contains(&version["references"], id)
}

fn contains(value: &Value, id: &str) -> bool {
  match value {
    Value::String(text) => text == id,
    Value::Array(items) => items.iter().any(|item| contains(item, id)),
    Value::Object(fields) => fields.values().any(|field| contains(field, id)),
    _ => false,
  }
}
