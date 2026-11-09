//! What the Agents hub of a local project lists (`Agents · v2` `acFkz`, L1–L14; `docs/design/team-configuration.md`
//! rules 11 and 12): the readiness of its profiles and tools ([`crate::readiness`]), its local MCP servers each on
//! their own, its instructions and skills, which need nothing of a computer, and what was archived. Nothing here is the
//! team's: no split, no versions of an MCP server (a local one is a connection, edited in place).

use std::path::Path;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, Connection, ConnectionKind, ConnectionStatus, REASONING_EFFORT,
    mcp::{McpServer, McpTransport},
  },
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, Draft, Instruction, Pipeline, ResourceKind, Skill, Tool, ToolKind, VersionNumber},
  secret::{SecretName, SecretVault},
  store::{ConnectionStore, McpServerStore, ResourceStore, SecretStore, TaskStore},
  time::Timestamp,
};
use orchester_store::{ArchiveStop, ProjectDb, StoreError};

use super::{
  form::{effort_options, model_options},
  team_hub::Form,
  team_profile::ConnectionChoice,
};
use crate::{
  connections::mcp::McpServerText,
  local_archive::Archivable,
  readiness::{LocalMcpReadiness, ProjectReadiness, ReadinessError},
  workspace::Workspace,
};

/// A resource of the project that needs nothing of this computer to be ready: an instruction or a skill, or a profile
/// or a tool that has only a draft so far, which no pipeline can run yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalPlain {
  pub id: ResourceId,
  pub kind: ResourceKind,
  pub name: String,
  /// The latest published version; `None` while it has only a draft.
  pub number: Option<VersionNumber>,
}

/// What a row and a panel of a local profile say beyond how it stands: the CLI it runs with, the connection it runs on,
/// the MCP servers it chooses and the pipelines whose steps run it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalProfile {
  /// The CLI it runs with: its connection's, and while that is gone the one its model belongs to
  /// ([`AgentProvider::cli_of_model`]); `None` when the connection is no agent connection or the model names no CLI.
  pub cli: Option<AgentProvider>,
  /// The connection it runs on, by its name; `None` when the project no longer has it.
  pub connection: Option<String>,
  pub mcp_servers: Vec<(ConnectionId, String)>,
  /// The pipelines, by name, whose latest version has a step that runs it.
  pub pipelines: Vec<String>,
  pub model: String,
  /// The reasoning effort it sets; `None` leaves it to the CLI.
  pub effort: Option<String>,
  /// The connections of this project that run its CLI, by name.
  pub connections: Vec<ConnectionChoice>,
  /// The connection it names, while it is one of [`Self::connections`].
  pub current: Option<ConnectionId>,
  /// When its latest version was published.
  pub published_at: Timestamp,
  /// Whether it has edits that are not published; what the fields above say is then the draft's.
  pub draft: bool,
  /// When its draft was last saved; `None` without one.
  pub draft_saved_at: Option<Timestamp>,
  /// What the selects of its panel offer ([`ProfileOptions`]).
  pub options: ProfileOptions,
  /// The project secrets the MCP servers it chooses take their values from, by name.
  pub secrets: Vec<SecretName>,
}

/// What the selects of a local profile's panel offer besides the choices it has made.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileOptions {
  /// The CLIs the project has a connection for, and the profile's own.
  pub clis: Vec<AgentProvider>,
  /// The models its connection offers, and its own while the connection does not list it; none while the connection
  /// is gone.
  pub models: Vec<String>,
  /// The reasoning efforts its connection lists for its model.
  pub efforts: Vec<String>,
  /// The project's MCP servers, by name, that it does not choose and that are not archived.
  pub servers: Vec<(ConnectionId, String)>,
}

/// A local resource or MCP server that was archived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalArchived {
  pub what: Archivable,
  /// What it is: a resource's kind, or [`ResourceKind::McpServer`].
  pub kind: ResourceKind,
  pub name: String,
  pub at: Timestamp,
  /// The runs its archive stopped, oldest first.
  pub stopped: Vec<StoppedRun>,
  /// An MCP server's fields as its panel reads them, which it keeps showing read-only.
  pub details: Option<McpServerText>,
}

/// A run an archive stopped, by its task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoppedRun {
  /// The task's key, *KON-9*.
  pub key: String,
  pub title: String,
}

/// The resources the hub of a local project lists, and how they stand on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalHub {
  readiness: ProjectReadiness,
  mcp_servers: Vec<LocalMcpReadiness>,
  plain: Vec<LocalPlain>,
  profiles: Vec<(ResourceId, LocalProfile)>,
  tool_forms: Vec<(ResourceId, Form)>,
  archived: Vec<LocalArchived>,
}

impl LocalHub {
  /// The profiles and tools with how they stand here, and the pipelines.
  pub const fn readiness(&self) -> &ProjectReadiness {
    &self.readiness
  }

  /// The local MCP servers that are not archived, by name.
  pub fn mcp_servers(&self) -> &[LocalMcpReadiness] {
    &self.mcp_servers
  }

  pub fn mcp_server(&self, id: ConnectionId) -> Option<&LocalMcpReadiness> {
    self.mcp_servers.iter().find(|found| found.id() == id)
  }

  /// The instructions, then the skills, then the profiles and tools that have only a draft, each by name.
  pub fn plain(&self) -> &[LocalPlain] {
    &self.plain
  }

  pub fn profile(&self, id: ResourceId) -> Option<&LocalProfile> {
    self
      .profiles
      .iter()
      .find_map(|(found, profile)| (*found == id).then_some(profile))
  }

  /// What a tool is; `None` for another resource.
  pub fn tool_form(&self, id: ResourceId) -> Option<Form> {
    self
      .tool_forms
      .iter()
      .find_map(|(found, form)| (*found == id).then_some(*form))
  }

  /// What was archived, newest first.
  pub fn archived(&self) -> &[LocalArchived] {
    &self.archived
  }

  /// The names of the profiles that choose the MCP server `id`, by name.
  pub fn profiles_choosing(&self, id: ConnectionId) -> Vec<&str> {
    let mut names: Vec<&str> = self
      .readiness
      .resources()
      .iter()
      .filter(|found| found.kind() == ResourceKind::AgentProfile)
      .filter(|found| {
        self
          .profile(found.resource())
          .is_some_and(|profile| profile.mcp_servers.iter().any(|(server, _)| *server == id))
      })
      .map(|found| found.name())
      .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names
  }
}

impl Workspace {
  /// The hub of the local project at `repo_root`. Reads the database and the keychain's list of what it has, so it
  /// blocks: run it on the background pool, on an explicit change and never on a timer
  /// ([`Workspace::project_readiness`]).
  pub fn local_hub(&self, repo_root: &Path, vault: &dyn SecretVault) -> Result<LocalHub, ReadinessError> {
    let db = ProjectDb::open(repo_root)?;
    let readiness = self.readiness_in(&db, repo_root, vault)?;
    Ok(LocalHub {
      readiness,
      mcp_servers: crate::readiness::local_mcp_servers(&db, repo_root, vault)?,
      plain: plain(&db)?,
      profiles: profiles(&db)?,
      tool_forms: tool_forms(&db)?,
      archived: archived(&db)?,
    })
  }
}

fn plain(db: &ProjectDb) -> Result<Vec<LocalPlain>, StoreError> {
  let mut plain = Vec::new();
  for kind in [
    ResourceKind::Instruction,
    ResourceKind::Skill,
    ResourceKind::AgentProfile,
    ResourceKind::Tool,
  ] {
    let mut of_kind = Vec::new();
    for resource in db.resources(kind)? {
      let number = match kind {
        ResourceKind::Instruction => db.latest_version::<Instruction>(resource.id())?.map(|v| v.number()),
        ResourceKind::Skill => db.latest_version::<Skill>(resource.id())?.map(|v| v.number()),
        ResourceKind::AgentProfile => db.latest_version::<AgentProfile>(resource.id())?.map(|v| v.number()),
        _ => db.latest_version::<Tool>(resource.id())?.map(|v| v.number()),
      };
      // A profile or tool with a version is listed with how it stands here (`readiness`).
      let draft_only = number.is_none();
      if draft_only || matches!(kind, ResourceKind::Instruction | ResourceKind::Skill) {
        of_kind.push(LocalPlain {
          id: resource.id(),
          kind,
          name: resource.name().to_string(),
          number,
        });
      }
    }
    of_kind.sort_by_key(|found| found.name.to_lowercase());
    plain.extend(of_kind);
  }
  Ok(plain)
}

fn profiles(db: &ProjectDb) -> Result<Vec<(ResourceId, LocalProfile)>, StoreError> {
  // The archived ones too: a profile still chooses one until it is restored or the profile drops it.
  let servers = db.all_mcp_servers()?;
  let available = db.mcp_servers()?;
  let connections = db.connections()?;
  let pipelines = latest_pipelines(db)?;
  let mut profiles = Vec::new();
  for resource in db.resources(ResourceKind::AgentProfile)? {
    let Some(version) = db.latest_version::<AgentProfile>(resource.id())? else {
      continue;
    };
    let draft = db.draft::<AgentProfile>(resource.id())?;
    // What the panel shows is the profile as it is now: its draft when it has one.
    let body = draft.as_ref().map_or_else(|| version.body(), |draft| draft.body());
    let connection = connections.iter().find(|found| found.id() == body.connection());
    let cli = match connection.map(orchester_core::connection::Connection::kind) {
      Some(ConnectionKind::Agent(cli)) => Some(*cli),
      Some(_) => None,
      None => AgentProvider::cli_of_model(body.model().as_str()),
    };
    let mut running: Vec<String> = pipelines
      .iter()
      .filter(|(_, pipeline)| pipeline.steps_run_by(resource.id()).next().is_some())
      .map(|(name, _)| name.clone())
      .collect();
    running.sort_by_key(|name| name.to_lowercase());
    let chosen: Vec<_> = body
      .mcp_servers()
      .ids()
      .iter()
      .filter_map(|id| servers.iter().find(|server| server.id() == *id))
      .collect();
    let of_kind: Vec<ConnectionChoice> = connections
      .iter()
      .filter(|found| matches!(found.kind(), ConnectionKind::Agent(provider) if Some(*provider) == cli))
      .map(|found| ConnectionChoice {
        id: found.id(),
        name: found.name().to_string(),
        version: found.version().map(str::to_owned),
        signed_out: found.status() == ConnectionStatus::SignInRequired,
      })
      .collect();
    profiles.push((
      resource.id(),
      LocalProfile {
        cli,
        connection: connection.map(|connection| connection.name().to_string()),
        mcp_servers: chosen
          .iter()
          .map(|server| (server.id(), server.name().to_string()))
          .collect(),
        pipelines: running,
        model: body.model().as_str().to_owned(),
        effort: body
          .settings()
          .iter()
          .find_map(|(key, value)| (key == REASONING_EFFORT).then(|| value.to_owned())),
        current: of_kind
          .iter()
          .find(|choice| choice.id == body.connection())
          .map(|choice| choice.id),
        connections: of_kind,
        published_at: version.published_at(),
        draft: draft.is_some(),
        draft_saved_at: draft.as_ref().map(Draft::updated_at),
        options: options(&connections, &available, (cli, body)),
        secrets: secret_names(db, &chosen)?,
      },
    ));
  }
  Ok(profiles)
}

/// What the selects of the panel of the profile `body`, which runs with `cli`, offer.
fn options(
  connections: &[Connection],
  available: &[McpServer],
  (cli, body): (Option<AgentProvider>, &AgentProfile),
) -> ProfileOptions {
  let mut clis: Vec<AgentProvider> = Vec::new();
  for connection in connections {
    if let ConnectionKind::Agent(provider) = connection.kind()
      && !clis.contains(provider)
    {
      clis.push(*provider);
    }
  }
  if let Some(cli) = cli
    && !clis.contains(&cli)
  {
    clis.push(cli);
  }
  let own = connections
    .iter()
    .find(|found| found.id() == body.connection())
    .and_then(|found| AgentConnection::try_from(found.clone()).ok());
  let (models, efforts) = own.as_ref().map_or_else(
    || (Vec::new(), Vec::new()),
    |connection| {
      (
        model_options(connection, Some(body.model()))
          .into_iter()
          .map(|option| option.id.as_str().to_owned())
          .collect(),
        effort_options(connection, body.model()),
      )
    },
  );
  ProfileOptions {
    clis,
    models,
    efforts,
    servers: available
      .iter()
      .filter(|server| !body.mcp_servers().contains(server.id()))
      .map(|server| (server.id(), server.name().to_string()))
      .collect(),
  }
}

/// The names of the project secrets the variables of `servers` take, each once, in the order the servers come.
fn secret_names(
  db: &ProjectDb,
  servers: &[&orchester_core::connection::mcp::McpServer],
) -> Result<Vec<SecretName>, StoreError> {
  let mut names: Vec<SecretName> = Vec::new();
  for id in servers.iter().flat_map(|server| server.secrets()) {
    if let Some(secret) = db.secret(id)?
      && !names.contains(secret.name())
    {
      names.push(secret.name().clone());
    }
  }
  Ok(names)
}

fn latest_pipelines(db: &ProjectDb) -> Result<Vec<(String, Pipeline)>, StoreError> {
  let mut found = Vec::new();
  for resource in db.resources(ResourceKind::Pipeline)? {
    if let Some(version) = db.latest_version::<Pipeline>(resource.id())? {
      found.push((resource.name().to_string(), version.body().clone()));
    }
  }
  Ok(found)
}

fn tool_forms(db: &ProjectDb) -> Result<Vec<(ResourceId, Form)>, StoreError> {
  let mut forms = Vec::new();
  for resource in db.resources(ResourceKind::Tool)? {
    if let Some(version) = db.latest_version::<Tool>(resource.id())? {
      forms.push((
        resource.id(),
        match version.body().kind() {
          ToolKind::Command => Form::ToolCommand,
          ToolKind::Http => Form::ToolHttp,
          ToolKind::Database => Form::ToolDatabase,
        },
      ));
    }
  }
  Ok(forms)
}

fn archived(db: &ProjectDb) -> Result<Vec<LocalArchived>, StoreError> {
  let mut archived = Vec::new();
  for kind in [
    ResourceKind::AgentProfile,
    ResourceKind::Instruction,
    ResourceKind::Skill,
    ResourceKind::Tool,
  ] {
    for (resource, at) in db.archived_local_resources(kind)? {
      archived.push(LocalArchived {
        what: Archivable::Resource(resource.id()),
        kind,
        name: resource.name().to_string(),
        at,
        stopped: stopped(db, &db.archive_stops_of_resource(resource.id())?)?,
        details: None,
      });
    }
  }
  let servers = db.all_mcp_servers()?;
  let secrets = db.secrets()?;
  for (id, at) in db.archived_mcp_servers()? {
    if let Some(server) = servers.iter().find(|server| server.id() == id) {
      archived.push(LocalArchived {
        what: Archivable::McpServer(id),
        kind: ResourceKind::McpServer,
        name: server.name().to_string(),
        at,
        stopped: stopped(db, &db.archive_stops_of_mcp_server(id)?)?,
        details: Some(McpServerText::of(server, &secrets)),
      });
    }
  }
  archived.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.name.cmp(&b.name)));
  Ok(archived)
}

/// The tasks of the runs `stops` names; a task the project no longer has is left out.
fn stopped(db: &ProjectDb, stops: &[ArchiveStop]) -> Result<Vec<StoppedRun>, StoreError> {
  let mut runs = Vec::new();
  for stop in stops {
    if let Some(task) = db.task(stop.task)? {
      runs.push(StoppedRun {
        key: task.key().to_string(),
        title: task.details().title.to_string(),
      });
    }
  }
  Ok(runs)
}

/// Whether a server is reached at an address, from its transport.
pub fn mcp_form(transport: &McpTransport) -> Form {
  match transport {
    McpTransport::Stdio(_) => Form::McpProgram,
    McpTransport::Http(_) => Form::McpRemote,
  }
}
