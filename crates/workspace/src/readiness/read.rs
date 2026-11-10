//! The read: gathers the needs, fills in `Here` and asks core's rule of each.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, ResourceKind, VersionNumber},
  secret::SecretVault,
  sharing::{Here, Needs, readiness},
  store::ResourceStore,
};
use orchester_store::ProjectDb;

use super::{
  ReadinessError, facts, local,
  needs::{self, Entry, Profile, Reviewer, Server},
  programs::McpPrograms,
  project::{PipelineReadiness, ProjectReadiness, ResourceReadiness},
  standing::{ReadinessStanding, Undecided},
  team,
};
use crate::team_config::TeamConfig;

/// What the read needs besides the project's database.
pub(super) struct Setting<'a> {
  pub(super) repo_root: &'a Path,
  pub(super) vault: &'a dyn SecretVault,
  /// For a team project, the program paths set for its team server's MCP servers and its configuration as the server
  /// was read; `None` for a local project.
  pub(super) team: Option<(McpPrograms, &'a TeamConfig)>,
}

/// The resources of the project, before they are checked against this computer.
struct Gathered {
  servers: Vec<Server>,
  profiles: Vec<Profile>,
  reviewers: Vec<Reviewer>,
  tools: Vec<Entry>,
  programs: McpPrograms,
}

fn gather(db: &ProjectDb, setting: Setting<'_>) -> Result<Gathered, ReadinessError> {
  let Some((programs, config)) = setting.team else {
    return Ok(Gathered {
      servers: Vec::new(),
      profiles: local::profiles(db, setting.repo_root)?,
      // A local project's reviewer is a setting of the project, not a resource (`orchester_core::run::review`).
      reviewers: Vec::new(),
      tools: local::tools(db, setting.vault)?,
      programs: McpPrograms::default(),
    });
  };
  let servers = needs::servers(config);
  let needed: Vec<_> = servers.iter().filter_map(|server| server.need.clone()).collect();
  Ok(Gathered {
    profiles: team::profiles(db, config, &needed)?,
    reviewers: team::reviewers(db, config)?,
    tools: team::tools(config),
    servers,
    programs,
  })
}

pub(super) fn read(db: &ProjectDb, setting: Setting<'_>) -> Result<ProjectReadiness, ReadinessError> {
  let vault = setting.vault;
  let config = setting.team.as_ref().map(|(_, config)| *config);
  let gathered = gather(db, setting)?;
  let server_ids: Vec<ResourceId> = gathered.servers.iter().map(|server| server.entry.resource).collect();
  let mut here = facts::with_programs(facts::base(db, vault)?, &gathered.programs, &server_ids);
  let mut entries: Vec<Entry> = Vec::new();
  for mut server in gathered.servers {
    match db.team_mcp_switch(server.entry.resource)? {
      Some(true) => here = here.with_mcp_on(server.entry.resource),
      Some(false) => server.entry.off_here = true,
      // Nobody decided here: not on, and it needs the person (its standing is missing `McpOff`).
      None => {}
    }
    entries.push(server.entry);
  }
  for profile in gathered.profiles {
    if let Some(pick) = profile.pick {
      here = here.with_pick(profile.entry.resource, pick);
    }
    entries.push(profile.entry);
  }
  for reviewer in gathered.reviewers {
    if let Some(pick) = reviewer.pick {
      here = here.with_pick(reviewer.entry.resource, pick);
    }
    if let Some(pick) = reviewer.decision_pick {
      here = here.with_decision_pick(reviewer.entry.resource, pick);
    }
    entries.push(reviewer.entry);
  }
  entries.extend(gathered.tools);
  let here = entries
    .iter()
    .flat_map(|entry| entry.found.iter().copied())
    .fold(here, |here, found| found.add_to(here));
  let pipelines = match config {
    Some(config) => team_pipelines(config, &entries),
    None => pipelines(db, &entries)?,
  };
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

/// Profiles, then MCP servers, then tools, then reviewers.
const fn kind_rank(kind: ResourceKind) -> u8 {
  match kind {
    ResourceKind::AgentProfile => 0,
    ResourceKind::McpServer => 1,
    ResourceKind::TeamReviewer => 3,
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

/// The team's pipelines, each read from its latest version; one whose latest version is not a pipeline this computer
/// reads is undecided.
fn team_pipelines(config: &TeamConfig, entries: &[Entry]) -> Vec<PipelineEntry> {
  let archived: Vec<(ResourceId, String)> = [ResourceKind::AgentProfile, ResourceKind::Tool]
    .into_iter()
    .flat_map(|kind| config.archived_of(kind))
    .map(|resource| (resource.id, resource.name.as_str().to_owned()))
    .collect();
  let mut pipelines = Vec::new();
  for resource in config.resources(ResourceKind::Pipeline) {
    let (Some(number), Some(body)) = (resource.number(), resource.body::<Pipeline>()) else {
      continue;
    };
    pipelines.push(PipelineEntry {
      resource: resource.id,
      name: resource.name.as_str().to_owned(),
      number,
      needs: body
        .map_err(|_| Undecided::NotTeamForm)
        .and_then(|pipeline| union(&pipeline, (entries, &archived))),
    });
  }
  pipelines
}

fn pipelines(db: &ProjectDb, entries: &[Entry]) -> Result<Vec<PipelineEntry>, ReadinessError> {
  let archived = archived_executors(db)?;
  let mut pipelines = Vec::new();
  for resource in db.resources(ResourceKind::Pipeline)? {
    let Some(version) = db.latest_version::<Pipeline>(resource.id())? else {
      continue;
    };
    pipelines.push(PipelineEntry {
      resource: resource.id(),
      name: resource.name().as_str().to_owned(),
      number: version.number(),
      needs: union(version.body(), (entries, &archived)),
    });
  }
  Ok(pipelines)
}

/// The needs of every profile or tool `pipeline` runs, in step order and then the integrator. One that is undecided, or
/// that this computer has no version of, decides the pipeline: it cannot start, and the first such member is named
/// ([`Undecided::Member`]). The other members' missing parts are not read while one is undecided.
fn union(pipeline: &Pipeline, (entries, archived): (&[Entry], &[(ResourceId, String)])) -> Result<Needs, Undecided> {
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
      .ok_or_else(|| unlisted(executor, archived))?;
    parts.push(entry.needs.clone().map_err(|why| Undecided::Member {
      id: executor,
      why: Box::new(why),
    })?);
  }
  Ok(Needs::union(parts))
}

/// The profiles and tools of a local project archived here, which a step may still name: a pipeline keeps naming what
/// is archived.
fn archived_executors(db: &ProjectDb) -> Result<Vec<(ResourceId, String)>, ReadinessError> {
  let mut archived = Vec::new();
  for kind in [ResourceKind::AgentProfile, ResourceKind::Tool] {
    for (resource, _) in db.archived_local_resources(kind)? {
      archived.push((resource.id(), resource.name().as_str().to_owned()));
    }
  }
  Ok(archived)
}

/// Why a step's executor has no entry: it was archived (and is named), or this computer has no version of it.
fn unlisted(executor: ResourceId, archived: &[(ResourceId, String)]) -> Undecided {
  archived
    .iter()
    .find(|(id, _)| *id == executor)
    .map_or(Undecided::RunsUnknown(executor), |(id, name)| {
      Undecided::RunsArchived(*id, name.clone())
    })
}
