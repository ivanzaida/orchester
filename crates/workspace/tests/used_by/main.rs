//! What uses a resource, read before it is archived (decision E of `docs/design/team-configuration.md`): one read
//! gives a local project and a team project the same shape, the other resources whose latest version names the
//! resource and the runs that have not ended and pin it. A local project computes it from its database ([`local`]), a
//! team project asks its server, here the stand-in ([`team`]).
//!
//! `cargo test -p orchester-workspace --test used_by used_by_`

mod local;
mod local_mcp;
mod mcp_servers;
#[path = "../support/mod.rs"]
mod support;
mod team;

use std::process::Command;

use orchester_api::{SharedResourceKind, UsedBy};
use tempfile::TempDir;

/// A folder with an empty git repository, which a project needs to be created in.
fn repository() -> TempDir {
  let dir = TempDir::new().expect("folder");
  let output = Command::new("git")
    .args(["init", "-b", "main"])
    .current_dir(dir.path())
    .output()
    .expect("git");
  assert!(
    output.status.success(),
    "git init: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  dir
}

/// The users of a resource as the confirmation shows them: resources as kind, name and version, runs as task key.
type Shown<'a> = (Vec<(SharedResourceKind, &'a str, u32)>, Vec<&'a str>);

/// A used-by answer as the Archive confirmation shows it: each resource as kind, name and version, each run as task
/// key.
fn shown(used: &UsedBy) -> Shown<'_> {
  (
    used
      .resources
      .iter()
      .map(|resource| (resource.kind, resource.name.as_str(), resource.version.get()))
      .collect(),
    used.runs.iter().map(|run| run.task_key.as_str()).collect(),
  )
}

/// What the tool "Deploy" is used by in the scenario both projects are built with: two pipelines running it, by name
/// without regard to case, and three runs that have not ended, by the task key's number (two runs of KON-3).
fn tool_users() -> Shown<'static> {
  (
    vec![
      (SharedResourceKind::Pipeline, "alpha flow", 1),
      (SharedResourceKind::Pipeline, "Delivery", 1),
    ],
    vec!["KON-3", "KON-3", "KON-12"],
  )
}
