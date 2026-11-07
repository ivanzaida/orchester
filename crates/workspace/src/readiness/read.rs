//! The read: gathers the needs, fills in `Here` and asks core's rule of each.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, ResourceKind, VersionNumber},
  secret::SecretVault,
  sharing::{Here, Needs, readiness},
  store::ResourceStore,
};
use orchester_store::{McpServerProgram, ProjectDb};

use super::{
  ReadinessError, facts, local,
  needs::{self, Entry, Profile, Server},
  project::{PipelineReadiness, ProjectReadiness, ResourceReadiness},
  standing::{ReadinessStanding, Undecided},
  team,
};

/// What the read needs besides the project's database.
pub(super) struct Setting<'a> {
  pub(super) repo_root: &'a Path,
  pub(super) vault: &'a dyn SecretVault,
  /// The program paths set for the team server's MCP servers when the project is shared with one; `None` for a local
  /// project.
  pub(super) team_programs: Option<Vec<McpServerProgram>>,
}

/// The resources of the project, before they are checked against this computer.
struct Gathered {
  servers: Vec<Server>,
  profiles: Vec<Profile>,
  tools: Vec<Entry>,
  programs: Vec<McpServerProgram>,
}

fn gather(db: &ProjectDb, setting: Setting<'_>) -> Result<Gathered, ReadinessError> {
  let Some(programs) = setting.team_programs else {
    return Ok(Gathered {
      servers: Vec::new(),
      profiles: local::profiles(db, setting.repo_root)?,
      tools: local::tools(db, setting.vault)?,
      programs: Vec::new(),
    });
  };
  let servers = needs::servers(db)?;
  let needed: Vec<_> = servers.iter().map(|server| server.need.clone()).collect();
  Ok(Gathered {
    profiles: team::profiles(db, &needed)?,
    tools: team::tools(db)?,
    servers,
    programs,
  })
}

pub(super) fn read(db: &ProjectDb, setting: Setting<'_>) -> Result<ProjectReadiness, ReadinessError> {
  let gathered = gather(db, setting)?;
  let server_ids: Vec<ResourceId> = gathered.servers.iter().map(|server| server.entry.resource).collect();
  let mut here = facts::with_programs(facts::base(db)?, gathered.programs, &server_ids);
  let mut entries: Vec<Entry> = Vec::new();
  for mut server in gathered.servers {
    if db.team_mcp_on_here(server.entry.resource)? {
      here = here.with_mcp_on(server.entry.resource);
    } else {
      server.entry.off_here = true;
    }
    entries.push(server.entry);
  }
  for profile in gathered.profiles {
    if let Some(pick) = profile.pick {
      here = here.with_pick(profile.entry.resource, pick);
    }
    entries.push(profile.entry);
  }
  entries.extend(gathered.tools);
  let here = entries
    .iter()
    .flat_map(|entry| entry.found.iter().copied())
    .fold(here, |here, found| found.add_to(here));
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

fn standing(needs: &Result<Needs, Undecided>, here: &Here) -> ReadinessStanding {
  match needs {
    Ok(needs) => ReadinessStanding::of(readiness(needs, here)),
    Err(undecided) => ReadinessStanding::Undecided(undecided.clone()),
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
  number: VersionNumber,
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
/// that this computer has no version of, decides the pipeline: it cannot start, and the first such member is named
/// ([`Undecided::Member`]). The other members' missing parts are not read while one is undecided.
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
    parts.push(entry.needs.clone().map_err(|why| Undecided::Member {
      id: executor,
      why: Box::new(why),
    })?);
  }
  Ok(Needs::union(parts))
}
