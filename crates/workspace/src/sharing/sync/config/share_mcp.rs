//! Share's MCP servers (`docs/design/team-configuration.md`, edge cases 1 and 12): each local MCP server an uploaded
//! profile chooses becomes a team MCP server, its definition without the program's path, and two with the same name
//! become one. A team MCP server takes the id of the local server, so the sharer's own profiles, which choose servers
//! by that id, keep choosing it; a duplicate's id maps to the one kept. This computer's own fill-ins come from the
//! same plan ([`super::share_fill_ins`]): the program's path, the secrets the server sends, and whether it is on.

use std::{
  collections::{BTreeMap, BTreeSet, HashMap},
  path::PathBuf,
};

use orchester_api::{SharedResourceKind, UploadResource, UploadVersion};
use orchester_core::{
  connection::mcp::{McpAuthorization, McpHeaderName, McpServer, McpServerSelection, McpTransport},
  id::{ConnectionId, ResourceId},
  resource::{
    AgentProfile, CommandProgram, ProgramLocation, ResourceKind, Slug, TeamMcpLaunch, TeamMcpProgram, TeamMcpRemote,
    TeamMcpServer, VersionNumber,
  },
  secret::Secret,
  store::{McpServerStore, ResourceStore, SecretStore},
};
use orchester_store::{ProjectDb, StoreError};

use crate::sharing::{ShareRefusal, ShareRefusalReason, SharingError, TeamMcpProblem};

/// One local MCP server as the team's resource.
#[derive(Clone, Debug)]
pub(super) struct PlannedServer {
  /// The team resource's id: the local server's.
  pub resource: ResourceId,
  pub body: TeamMcpServer,
  pub slug: Slug,
  pub created_at: i64,
  /// The program's path here, when the local server started a program by its absolute path.
  pub program_path: Option<PathBuf>,
  /// Whether the server is on here.
  pub enabled: bool,
  /// The local secrets the server sends, whose values this computer fills in under their names.
  pub secrets: Vec<Secret>,
}

/// The team MCP servers Share uploads, and what each local server maps to.
#[derive(Debug, Default)]
pub(super) struct McpPlan {
  servers: Vec<PlannedServer>,
  by_local: HashMap<ConnectionId, ResourceId>,
  /// Servers that cannot be team ones. They still map, so a profile choosing one is not refused again.
  pub refusals: Vec<ShareRefusal>,
}

impl McpPlan {
  /// The MCP servers the profiles of `db` choose in any published version, planned as team ones.
  pub fn read(db: &ProjectDb) -> Result<Self, SharingError> {
    let used = used_servers(db)?;
    let secrets = db.secrets()?;
    let mut local: Vec<McpServer> = db
      .mcp_servers()?
      .into_iter()
      .filter(|server| used.contains(&server.id()))
      .collect();
    local.sort_by_key(|server| (server.created_at(), server.id().to_string()));
    let mut plan = Self::default();
    let mut kept: BTreeMap<String, ResourceId> = BTreeMap::new();
    for server in &local {
      let name = server.name().as_str().to_owned();
      if let Some(resource) = kept.get(&name) {
        plan.by_local.insert(server.id(), *resource);
        continue;
      }
      let resource = ResourceId::from_uuid(server.id().as_uuid());
      kept.insert(name, resource);
      plan.by_local.insert(server.id(), resource);
      plan.add(server, resource, &secrets)?;
    }
    Ok(plan)
  }

  fn add(&mut self, server: &McpServer, resource: ResourceId, secrets: &[Secret]) -> Result<(), SharingError> {
    let name = server.name().as_str().to_owned();
    match team_server(server) {
      Ok((body, program_path)) => {
        let sends = server
          .secrets()
          .into_iter()
          .map(|id| {
            secrets
              .iter()
              .find(|secret| secret.id() == id)
              .cloned()
              .ok_or(StoreError::SecretNotFound(id))
          })
          .collect::<Result<Vec<_>, _>>()?;
        let slug = self.slug(&name)?;
        self.servers.push(PlannedServer {
          resource,
          body,
          slug,
          created_at: server.created_at().unix_millis(),
          program_path,
          enabled: server.enabled(),
          secrets: sends,
        });
      }
      Err(problem) => self.refusals.push(ShareRefusal {
        resource,
        name: name.clone(),
        reason: ShareRefusalReason::McpServer { server: name, problem },
      }),
    }
    Ok(())
  }

  /// A slug for the server `name` that no other uploaded MCP server has (slugs are told apart per kind, and a local
  /// project has no MCP server resources): names allow `_`, slugs only `-`.
  fn slug(&self, name: &str) -> Result<Slug, SharingError> {
    let base = name.replace('_', "-");
    let mut candidate = base.clone();
    for suffix in 2.. {
      if !self.servers.iter().any(|server| server.slug.as_str() == candidate) {
        break;
      }
      candidate = format!("{base}-{suffix}");
    }
    Slug::new(&candidate).map_err(|_| SharingError::InvalidAnswer("MCP server slug"))
  }

  pub fn servers(&self) -> &[PlannedServer] {
    &self.servers
  }

  /// The team resource every server this plan covers maps to, with the refused ones.
  pub fn team_ids(&self) -> Vec<ResourceId> {
    let mut ids: Vec<ResourceId> = self.by_local.values().copied().collect();
    ids.sort_by_key(ToString::to_string);
    ids.dedup();
    ids
  }

  /// `profile` choosing the team's MCP servers: each local server it chooses as the team resource it became. In the
  /// newest version a server this computer no longer has stays chosen, and the profile is refused for it; in an older
  /// one, which is history the team does not check, it is left out.
  pub fn adjusted(&self, profile: &AgentProfile, newest: bool) -> AgentProfile {
    let mut chosen: Vec<ConnectionId> = Vec::new();
    for local in profile.mcp_servers().ids() {
      let mapped = match self.by_local.get(local) {
        Some(team) => ConnectionId::from_uuid(team.as_uuid()),
        None if newest => *local,
        None => {
          tracing::warn!(server = %local, "an older version of a profile chooses an MCP server this computer no longer has; the shared copy leaves it out");
          continue;
        }
      };
      if !chosen.contains(&mapped) {
        chosen.push(mapped);
      }
    }
    profile
      .clone()
      .with_mcp_servers(McpServerSelection::new(chosen).expect("distinct servers, no more than the profile chose"))
  }

  /// The uploads of the planned servers, each with one version 1 as it was made here.
  pub fn uploads(&self) -> Result<Vec<UploadResource>, SharingError> {
    self
      .servers
      .iter()
      .map(|server| {
        let body = serde_json::to_value(&server.body).map_err(|error| StoreError::Corrupt {
          what: "team MCP server",
          detail: error.to_string(),
        })?;
        Ok(UploadResource {
          id: server.resource,
          kind: SharedResourceKind::McpServer,
          slug: server.slug.as_str().to_owned(),
          name: server.body.name().as_str().to_owned(),
          archived: false,
          versions: vec![UploadVersion {
            number: VersionNumber::FIRST,
            references: Vec::new(),
            body,
            published_at: server.created_at,
          }],
        })
      })
      .collect()
  }
}

/// The local MCP servers any published version of any profile of `db` chooses.
fn used_servers(db: &ProjectDb) -> Result<BTreeSet<ConnectionId>, StoreError> {
  let mut used = BTreeSet::new();
  for profile in db.resources(ResourceKind::AgentProfile)? {
    for version in db.versions::<AgentProfile>(profile.id())? {
      used.extend(version.body().mcp_servers().ids().iter().copied());
    }
  }
  Ok(used)
}

/// The team's form of `server` and the path of its program here.
fn team_server(server: &McpServer) -> Result<(TeamMcpServer, Option<PathBuf>), TeamMcpProblem> {
  let (launch, path) = match server.transport() {
    McpTransport::Stdio(stdio) => {
      let (program, path) = program_name(stdio.program())?;
      let variables = server.environment().iter().map(|(name, _)| name.clone()).collect();
      let launch = TeamMcpProgram::new(
        program,
        stdio.args().to_vec(),
        stdio.working_directory().cloned(),
        variables,
      )?;
      (TeamMcpLaunch::Program(launch), path)
    }
    McpTransport::Http(http) => {
      let headers = secret_headers(http.authorization())?;
      (
        TeamMcpLaunch::Remote(TeamMcpRemote::new(http.url().clone(), headers)?),
        None,
      )
    }
  };
  Ok((
    TeamMcpServer::new(server.name().clone(), launch, server.exempt_tools().clone()),
    path,
  ))
}

/// The program the team names, and the path it has here: a name on the search path stays as it is with no path, an
/// absolute path becomes its file name and is kept as this computer's path.
fn program_name(program: &CommandProgram) -> Result<(CommandProgram, Option<PathBuf>), TeamMcpProblem> {
  match program.location() {
    ProgramLocation::OnPath(_) => Ok((program.clone(), None)),
    ProgramLocation::InWorktree(_) => Err(TeamMcpProblem::ProgramInWorktree {
      program: program.as_str().to_owned(),
    }),
    ProgramLocation::Absolute(path) => {
      let unusable = || TeamMcpProblem::ProgramName {
        program: program.as_str().to_owned(),
      };
      let file = path.file_name().and_then(|file| file.to_str()).ok_or_else(unusable)?;
      let named = CommandProgram::new(file).map_err(|_| unusable())?;
      if matches!(named.location(), ProgramLocation::OnPath(_)) {
        Ok((named, Some(path.to_path_buf())))
      } else {
        Err(unusable())
      }
    }
  }
}

/// The headers a remote server is sent a secret in.
fn secret_headers(authorization: &McpAuthorization) -> Result<Vec<McpHeaderName>, TeamMcpProblem> {
  match authorization {
    McpAuthorization::None => Ok(Vec::new()),
    McpAuthorization::Bearer { .. } | McpAuthorization::BasicCredential { .. } => {
      Ok(vec![McpHeaderName::authorization()])
    }
    McpAuthorization::Header { name, .. } => Ok(vec![name.clone()]),
    McpAuthorization::Basic { .. } => Err(TeamMcpProblem::BasicUsername),
  }
}
