//! Share's MCP servers (`docs/design/team-configuration.md`, edge cases 1 and 12): each local MCP server an uploaded
//! profile chooses becomes a team MCP server, its definition without the program's path. A team MCP server takes the
//! id of the local server, so the sharer's own profiles, which choose servers by that id, keep choosing it. Names are
//! unique in a project (`mcp_servers.name UNIQUE`), so no two servers become one. This computer's own fill-ins come
//! from the same plan ([`super::share_fill_ins`]): the program's path, the values of the team secrets the server
//! takes, and whether it is on.
//!
//! A server only an older version of a profile chooses, and that cannot be a team one, does not refuse the Share: the
//! shared copy of that version leaves it out, and the Share says so ([`ShareNote`]).

mod team_form;

use std::collections::{BTreeSet, HashMap};

use orchester_api::{SharedResourceKind, UploadResource, UploadVersion};
use orchester_core::{
  connection::mcp::{McpServer, McpServerSelection},
  id::{ConnectionId, ResourceId},
  name::Name,
  resource::{AgentProfile, ResourceKind, Slug, VersionNumber},
  store::{McpServerStore, ResourceStore, SecretStore},
};
use orchester_store::{ProjectDb, StoreError};

pub use self::team_form::{FillSource, LocalProgram, SecretFill};
use self::team_form::{FormError, TeamForm, team_form};
use crate::sharing::{ShareNote, ShareRefusal, ShareRefusalReason, SharingError, TeamMcpProblem};

/// One local MCP server as the team's resource.
#[derive(Clone, Debug)]
pub struct PlannedServer {
  /// The team resource's id: the local server's.
  pub resource: ResourceId,
  pub body: orchester_core::resource::TeamMcpServer,
  pub slug: Slug,
  pub created_at: i64,
  /// How the local server started its program, for this computer's path.
  pub program: LocalProgram,
  /// Whether the server is on here.
  pub enabled: bool,
  /// The team secrets this computer gives a value, each from where the local server had it.
  pub fills: Vec<SecretFill>,
}

/// The team MCP servers Share uploads, and what each local server maps to.
#[derive(Debug, Default)]
pub struct McpPlan {
  servers: Vec<PlannedServer>,
  by_local: HashMap<ConnectionId, ResourceId>,
  /// Servers that cannot be team ones. A server the newest version of a profile chooses still maps, so a profile
  /// choosing one is not refused again.
  pub refusals: Vec<ShareRefusal>,
  /// Servers only older versions choose that cannot be team ones, by name.
  left_out: HashMap<ConnectionId, String>,
  notes: Vec<ShareNote>,
}

/// One published version of a profile and the local MCP servers it chooses.
struct Chosen {
  profile: String,
  version: VersionNumber,
  newest: bool,
  servers: Vec<ConnectionId>,
}

impl McpPlan {
  /// The MCP servers the profiles of `db` choose in any published version, planned as team ones.
  pub fn read(db: &ProjectDb) -> Result<Self, SharingError> {
    let chosen = chosen_servers(db)?;
    let secrets = db.secrets()?;
    let used: BTreeSet<ConnectionId> = chosen.iter().flat_map(|found| found.servers.iter().copied()).collect();
    let newest: BTreeSet<ConnectionId> = chosen
      .iter()
      .filter(|found| found.newest)
      .flat_map(|found| found.servers.iter().copied())
      .collect();
    let mut local: Vec<McpServer> = db
      .mcp_servers()?
      .into_iter()
      .filter(|server| used.contains(&server.id()))
      .collect();
    local.sort_by_key(|server| (server.created_at(), server.id().to_string()));
    let mut plan = Self::default();
    for server in &local {
      plan.consider(server, &secrets, newest.contains(&server.id()))?;
    }
    plan.note_variables();
    plan.note_left_out(&chosen);
    Ok(plan)
  }

  fn consider(
    &mut self,
    server: &McpServer,
    secrets: &[orchester_core::secret::Secret],
    in_newest: bool,
  ) -> Result<(), SharingError> {
    let resource = ResourceId::from_uuid(server.id().as_uuid());
    let problem = match team_form(server, secrets) {
      Ok(form) => match self.accept(server, resource, form) {
        Ok(()) => return Ok(()),
        Err(problem) => problem,
      },
      Err(FormError::Refused(problem)) => problem,
      Err(FormError::SecretMissing(id)) => return Err(StoreError::SecretNotFound(id).into()),
    };
    let name = server.name().as_str().to_owned();
    if in_newest {
      self.by_local.insert(server.id(), resource);
      self.refusals.push(ShareRefusal {
        resource,
        name: name.clone(),
        reason: ShareRefusalReason::McpServer { server: name, problem },
      });
    } else {
      self.left_out.insert(server.id(), name);
    }
    Ok(())
  }

  /// Plans `form` of `server`, or says why the team cannot take it next to the servers planned so far.
  fn accept(&mut self, server: &McpServer, resource: ResourceId, form: TeamForm) -> Result<(), TeamMcpProblem> {
    let name = Name::new(server.name().as_str()).expect("an MCP server name is at most 48 characters and not blank");
    let taken: Vec<Slug> = self.servers.iter().map(|planned| planned.slug.clone()).collect();
    let fallback = Slug::new("mcp-server").expect("a literal slug");
    let planned = PlannedServer {
      resource,
      body: form.body,
      slug: Slug::unique_for(&name, &fallback, &taken),
      created_at: server.created_at().unix_millis(),
      program: form.program,
      enabled: server.enabled(),
      fills: form.fills,
    };
    if let Some(taken) = self.secret_taken(&planned) {
      return Err(taken);
    }
    self.by_local.insert(server.id(), resource);
    self.servers.push(planned);
    Ok(())
  }

  /// The problem when `planned` fills a team secret from another place than a planned server already does: a team
  /// secret has one value on a computer.
  fn secret_taken(&self, planned: &PlannedServer) -> Option<TeamMcpProblem> {
    planned.fills.iter().find_map(|fill| {
      self.servers.iter().find_map(|other| {
        other
          .fills
          .iter()
          .any(|there| there.name == fill.name && there.source != fill.source)
          .then(|| TeamMcpProblem::SecretTaken {
            secret: fill.name.as_str().to_owned(),
            other: other.body.name().as_str().to_owned(),
          })
      })
    })
  }

  /// A stdio server's plain-text values become the values of team secrets: each member sets them.
  fn note_variables(&mut self) {
    for server in &self.servers {
      let variables: Vec<String> = server
        .fills
        .iter()
        .filter(|fill| matches!(fill.source, FillSource::Text(_)))
        .map(|fill| fill.name.as_str().to_owned())
        .collect();
      if !variables.is_empty() {
        self.notes.push(ShareNote::VariablesAreSecrets {
          server: server.body.name().as_str().to_owned(),
          variables,
        });
      }
    }
  }

  /// A server an older version chooses that the shared copy of that version leaves out.
  fn note_left_out(&mut self, chosen: &[Chosen]) {
    for found in chosen.iter().filter(|found| !found.newest) {
      for server in &found.servers {
        if self.by_local.contains_key(server) {
          continue;
        }
        let (profile, version) = (found.profile.clone(), found.version.get());
        self.notes.push(match self.left_out.get(server) {
          Some(server) => ShareNote::UnsharedServerLeftOut {
            profile,
            version,
            server: server.clone(),
          },
          None => ShareNote::GoneServerLeftOut { profile, version },
        });
      }
    }
  }

  pub fn servers(&self) -> &[PlannedServer] {
    &self.servers
  }

  /// What Share did to the MCP servers that the person should know.
  pub fn notes(&self) -> &[ShareNote] {
    &self.notes
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
  /// one, which is history the team does not check, it is left out (the plan notes it).
  pub fn adjusted(&self, profile: &AgentProfile, newest: bool) -> AgentProfile {
    let mut chosen: Vec<ConnectionId> = Vec::new();
    for local in profile.mcp_servers().ids() {
      let mapped = match self.by_local.get(local) {
        Some(team) => ConnectionId::from_uuid(team.as_uuid()),
        None if newest => *local,
        None => continue,
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

/// Every published version of every profile of `db` with the local MCP servers it chooses, oldest first.
fn chosen_servers(db: &ProjectDb) -> Result<Vec<Chosen>, StoreError> {
  let mut chosen = Vec::new();
  for profile in db.resources(ResourceKind::AgentProfile)? {
    let versions = db.versions::<AgentProfile>(profile.id())?;
    for (index, version) in versions.iter().enumerate() {
      chosen.push(Chosen {
        profile: profile.name().as_str().to_owned(),
        version: version.number(),
        newest: index + 1 == versions.len(),
        servers: version.body().mcp_servers().ids().to_vec(),
      });
    }
  }
  Ok(chosen)
}
