//! The read: gathers the needs, fills in `Here` and asks core's rule of each.

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, ResourceKind},
  sharing::{Here, Needs, readiness},
  store::ResourceStore,
};
use orchester_store::{McpServerProgram, ProjectDb};

use super::{
  ReadinessError, facts,
  needs::{self, Entry},
  project::{PipelineReadiness, ProjectReadiness, ResourceReadiness},
  standing::{Standing, Undecided},
};

pub(super) fn read(db: &ProjectDb, programs: Vec<McpServerProgram>) -> Result<ProjectReadiness, ReadinessError> {
  let team = db.sharing()?.is_some();
  let servers = if team { needs::servers(db)? } else { Vec::new() };
  let server_ids: Vec<ResourceId> = servers.iter().map(|server| server.entry.resource).collect();
  let mut here = facts::with_programs(facts::base(db)?, programs, &server_ids);
  let mut entries: Vec<Entry> = Vec::new();
  let mut needed = Vec::new();
  for mut server in servers {
    if db.team_mcp_on_here(server.entry.resource)? {
      here = here.with_mcp_on(server.entry.resource);
    } else {
      server.entry.off_here = true;
    }
    needed.push(server.need);
    entries.push(server.entry);
  }
  for profile in needs::profiles(db, team, &needed)? {
    if let Some(pick) = profile.pick {
      here = here.with_pick(profile.entry.resource, pick);
    }
    entries.push(profile.entry);
  }
  entries.extend(needs::tools(db)?);
  let pipelines = pipelines(db, &entries)?;
  entries.sort_by_key(|entry| kind_rank(entry.kind));
  Ok(ProjectReadiness {
    resources: entries.into_iter().map(|entry| resource(entry, &here)).collect(),
    pipelines: pipelines
      .into_iter()
      .map(|pipeline| PipelineReadiness {
        standing: standing(&pipeline.needs, &here),
        pipeline: pipeline.resource,
        name: pipeline.name,
        number: pipeline.number,
      })
      .collect(),
  })
}

/// Profiles, then MCP servers, then tools.
const fn kind_rank(kind: ResourceKind) -> u8 {
  match kind {
    ResourceKind::AgentProfile => 0,
    ResourceKind::McpServer => 1,
    _ => 2,
  }
}

fn standing(needs: &Result<Needs, Undecided>, here: &Here) -> Standing {
  match needs {
    Ok(needs) => Standing::of(readiness(needs, here)),
    Err(undecided) => Standing::Undecided(*undecided),
  }
}

fn resource(entry: Entry, here: &Here) -> ResourceReadiness {
  ResourceReadiness {
    standing: standing(&entry.needs, here),
    resource: entry.resource,
    kind: entry.kind,
    name: entry.name,
    number: entry.number,
    off_here: entry.off_here,
  }
}

/// A pipeline and what its steps ask: the union of the needs of what they run and of the Deliver step's integrator.
struct PipelineEntry {
  resource: ResourceId,
  name: String,
  number: orchester_core::resource::VersionNumber,
  needs: Result<Needs, Undecided>,
}

fn pipelines(db: &ProjectDb, entries: &[Entry]) -> Result<Vec<PipelineEntry>, ReadinessError> {
  let mut pipelines = Vec::new();
  for resource in db.resources(ResourceKind::Pipeline)? {
    let Some(version) = db.latest_version::<Pipeline>(resource.id())? else {
      continue;
    };
    pipelines.push(PipelineEntry {
      resource: resource.id(),
      name: resource.name().as_str().to_owned(),
      number: version.number(),
      needs: union(version.body(), entries),
    });
  }
  Ok(pipelines)
}

/// The needs of every profile or tool `pipeline` runs, in step order and then the integrator. One that is undecided, or
/// that this computer has no version of, decides the pipeline: it cannot start.
fn union(pipeline: &Pipeline, entries: &[Entry]) -> Result<Needs, Undecided> {
  let integrator = pipeline.deliver().and_then(|deliver| deliver.integrator());
  let executors = pipeline
    .steps()
    .iter()
    .filter_map(|step| step.kind.executor())
    .chain(integrator);
  let mut parts = Vec::new();
  for executor in executors {
    let entry = entries
      .iter()
      .find(|entry| entry.resource == executor)
      .ok_or(Undecided::RunsUnknown(executor))?;
    parts.push(entry.needs.clone()?);
  }
  Ok(Needs::union(parts))
}
