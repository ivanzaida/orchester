//! `GET …/resources/{r}/used-by` on the stand-in: the other resources that are not archived and whose latest version
//! names the resource, by name (case-insensitively) and then id, and the runs registered with
//! [`TeamServer::pin_run`] that have not ended and pin a version of it, by the task key's number and then run id.
//! A version names a resource by the typed fields `docs/cloud-api.md` lists, read with core's body types. The target
//! may be archived; it is never in its own `resources`.

use orchester_core::{
  id::{ResourceId, RunId},
  resource::{AgentTargets, Instruction, InstructionScope, Pipeline, Skill, Tool, VersionNumber},
};
use serde_json::{Value, json};

use super::{Project, Stored, error};
use crate::{
  Reference, ReferenceTarget, SharedResourceKind,
  test_support::team_server::{Person, TeamServer},
};

/// The last work report the server accepted for a run. The server knows a run only from its first accepted report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunReport {
  /// The run goes on, also while interrupted or waiting for a person on the desktop.
  Progress,
  Succeeded,
  Failed,
  Cancelled,
}

impl RunReport {
  const fn ends(self) -> bool {
    !matches!(self, Self::Progress)
  }
}

/// A run of a task of the project at some slug whose work report the server accepted, and whose execution bundle pins
/// versions of shared resources: the pipeline, a step's profile or tool, an instruction, and an MCP server the
/// bundle's profiles would name once a bundle carries them. The stand-in builds no bundle at `work.start`, so a test
/// names the pins the bundle would hold; the runs of one work share one bundle, so give them the same pins.
#[derive(Clone, Debug)]
pub struct PinnedRun {
  /// The key of the run's task, e.g. `KON-12`.
  pub task_key: String,
  pub run: RunId,
  /// The work the run belongs to, numbered within its task: runs reported later for the same work end the earlier
  /// ones, a run of another work of the same task does not.
  pub work: u32,
  /// Who holds the task's claim; `None` once it was released.
  pub holder: Option<Person>,
  /// The last accepted work report; a terminal one ends the run. A run with no accepted report is not known to the
  /// server, so a test does not pin it.
  pub last_report: RunReport,
  /// The versions its bundle pins.
  pub pins: Vec<(ResourceId, VersionNumber)>,
}

impl TeamServer {
  /// The project at `slug` has `run`, which counts in used-by while it has not ended and pins a version of the
  /// resource asked about. Runs are pinned in the order the server first heard of them: a run pinned later for the
  /// same task and work is a newer run of that work, which ends the earlier ones. Panics for a slug the stand-in does
  /// not hold or a task key that is not `KEY-<number>`, so a test cannot pin to nothing.
  pub fn pin_run(&self, slug: &str, run: PinnedRun) {
    task_number(&run.task_key);
    let mut state = self.state.lock();
    let id = project_id(&state.projects, slug);
    state.resources.projects.entry(id).or_default().runs.push(run);
  }

  /// The claim on the task of `run` in the project at `slug` is now `holder`'s (a take-over) or released (`None`).
  /// Panics for a run that was not pinned.
  pub fn claim_run_task(&self, slug: &str, run: RunId, holder: Option<Person>) {
    let mut state = self.state.lock();
    let id = project_id(&state.projects, slug);
    let pinned = state
      .resources
      .projects
      .entry(id)
      .or_default()
      .runs
      .iter_mut()
      .find(|pinned| pinned.run == run)
      .expect("`claim_run_task` names a run that was pinned");
    pinned.holder = holder;
  }
}

fn project_id(projects: &[Value], slug: &str) -> String {
  projects
    .iter()
    .find(|project| project["slug"] == slug)
    .and_then(|project| project["id"].as_str())
    .expect("the stand-in holds the project")
    .to_owned()
}

/// The number part of a task key: `12` of `KON-12`.
fn task_number(key: &str) -> u64 {
  key
    .rsplit_once('-')
    .and_then(|(_, number)| number.parse().ok())
    .expect("a task key is `KEY-<number>`")
}

impl Project {
  pub(super) fn used_by(&self, resource: &str) -> (u16, Value) {
    let Ok(target) = resource.parse::<ResourceId>() else {
      return (404, error("not_found"));
    };
    if !self.resources.iter().any(|stored| stored.id() == resource) {
      return (404, error("not_found"));
    }
    let mut resources: Vec<&Stored> = self
      .resources
      .iter()
      .filter(|stored| stored.id() != resource && stored.resource["archived"] == false)
      .filter(|stored| {
        stored
          .latest()
          .is_some_and(|latest| names(&stored.resource["kind"], latest, target))
      })
      .collect();
    resources.sort_by_cached_key(|stored| {
      (
        stored.resource["name"].as_str().unwrap_or_default().to_lowercase(),
        stored.id().to_string(),
      )
    });
    let mut runs: Vec<&PinnedRun> = self
      .runs
      .iter()
      .enumerate()
      .filter(|(place, run)| {
        let superseded = self.runs[place + 1..]
          .iter()
          .any(|newer| (&newer.task_key, newer.work) == (&run.task_key, run.work));
        !superseded && !run.last_report.ends() && run.pins.iter().any(|(pinned, _)| *pinned == target)
      })
      .map(|(_, run)| run)
      .collect();
    runs.sort_by_key(|run| (task_number(&run.task_key), run.run));
    (
      200,
      json!({
        "resources": resources.iter().map(|stored| json!({
          "kind": stored.resource["kind"], "id": stored.resource["id"], "name": stored.resource["name"],
          "version": stored.resource["latest"],
        })).collect::<Vec<_>>(),
        "runs": runs.iter().map(|run| {
          let mut read = json!({"task_key": run.task_key, "run": run.run});
          if let Some(holder) = run.holder {
            read["holder"] = holder.wire();
          }
          read
        }).collect::<Vec<_>>(),
      }),
    )
  }
}

/// Whether `version` of a resource of `kind` names `target`: a reference to it as a tool, or, by the kind's body, a
/// field that holds its server resource id. A body that is no body of its kind names nothing.
fn names(kind: &Value, version: &Value, target: ResourceId) -> bool {
  let referenced = serde_json::from_value::<Vec<Reference>>(version["references"].clone()).is_ok_and(|references| {
    references
      .iter()
      .any(|reference| reference.target == ReferenceTarget::Tool { id: target })
  });
  let body = version["body"].clone();
  referenced
    || match serde_json::from_value::<SharedResourceKind>(kind.clone()) {
      Ok(SharedResourceKind::Pipeline) => serde_json::from_value::<Pipeline>(body).is_ok_and(|pipeline| {
        pipeline.steps_run_by(target).next().is_some()
          || pipeline.deliver().and_then(|deliver| deliver.integrator()) == Some(target)
      }),
      Ok(SharedResourceKind::Instruction) => serde_json::from_value::<Instruction>(body).is_ok_and(|instruction| {
        matches!(instruction.targets(), AgentTargets::Profiles(profiles) if profiles.contains(&target))
          || scope_names(instruction.scope(), target)
      }),
      Ok(SharedResourceKind::Skill) => {
        serde_json::from_value::<Skill>(body).is_ok_and(|skill| scope_names(skill.scope(), target))
      }
      Ok(SharedResourceKind::Tool) => serde_json::from_value::<Tool>(body).is_ok_and(|tool| {
        tool
          .access()
          .is_some_and(|access| access.callers().contains_key(&target))
      }),
      Ok(SharedResourceKind::AgentProfile | SharedResourceKind::TeamReviewer | SharedResourceKind::McpServer)
      | Err(_) => false,
    }
}

fn scope_names(scope: &InstructionScope, target: ResourceId) -> bool {
  match scope {
    InstructionScope::Pipeline { pipeline } | InstructionScope::Step { pipeline, .. } => *pipeline == target,
    InstructionScope::Project | InstructionScope::AgentRole { .. } => false,
  }
}
