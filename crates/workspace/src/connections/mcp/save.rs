//! Storing the project's MCP servers, as the Connections screen and the setup tools change them: a new server or a
//! server's new settings, turning one on or off, and removing one no agent profile chooses. A server's program is
//! stored as the fixed file it is (`tools::program`), as a command tool's is. Which profiles choose a server is one
//! rule for the screen, the tools and removal ([`server_uses`]). Every function blocks on SQLite; the UI calls them
//! off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::mcp::{McpServer, McpServerError, McpTransport, StdioServer},
  id::{ConnectionId, ResourceId, SecretId},
  resource::{AgentProfile, ModelId, ResourceKind},
  secret::{self, SecretError, SecretName, SecretValue, SecretVault, VaultError},
  store::{McpServerStore, ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::{Allowed, McpServerDraft, NAME_TAKEN, widenings};
use crate::{
  messages::{self, Message},
  tools::program::{ProgramProblem, fixed_program},
};

/// Why a server was not stored or removed.
#[derive(Debug, Error)]
pub enum McpSaveError {
  #[error(transparent)]
  Store(StoreError),
  #[error(transparent)]
  Server(#[from] McpServerError),
  /// The program cannot be stored as a fixed file outside the repository.
  #[error("the MCP server's program cannot be stored: {0:?}")]
  Program(ProgramProblem),
  /// Agent profiles still choose the server, by their roles: a draft or the latest published version names it. They
  /// switch first.
  #[error("agent profiles still choose the MCP server")]
  InUse(Vec<Arc<str>>),
  /// The server changed since the save was asked for, so it widens what was not allowed: these reasons.
  #[error("the MCP server changed meanwhile")]
  Changed(Vec<Message>),
  /// The secret store did not keep the new value of a secret the server's variables take; the server is as it was.
  #[error("the secret store did not keep the value of {secret}: {source}")]
  SecretStore { secret: SecretName, source: VaultError },
  /// A value was to replace a secret the project does not have, or has archived.
  #[error("the secret {0} is gone")]
  SecretGone(SecretId),
  /// A value was given for a secret the server's variables and authorization do not take.
  #[error("the server does not take the secret {0}")]
  SecretNotTaken(SecretId),
  /// The save failed for `cause` after the server was stored, and putting it back as it was failed too: it keeps the
  /// settings of the save.
  #[error("{cause}; and the server could not be put back as it was: {restore}")]
  NotRestored {
    cause: Box<McpSaveError>,
    restore: StoreError,
  },
}

/// A store failure is logged where it arises, once, whichever way the save then says it; a name that is taken is the
/// person's to fix and is not.
impl From<StoreError> for McpSaveError {
  fn from(error: StoreError) -> Self {
    if !matches!(error, StoreError::DuplicateConnectionName(_)) {
      tracing::error!(%error, "cannot store the MCP server");
    }
    Self::Store(error)
  }
}

impl McpSaveError {
  pub fn message(&self) -> Message {
    match self {
      Self::Store(StoreError::DuplicateConnectionName(_)) => Message::new(NAME_TAKEN),
      Self::Store(error) => messages::store_error(error),
      Self::Server(McpServerError::EnvironmentOnHttp) => Message::new("connections.mcp.errors.variables"),
      Self::Program(problem) => program_problem_message(problem),
      Self::InUse(roles) => Message::new("connections.mcp.errors.inUse").with("profiles", roles.join(", ")),
      Self::Changed(_) => Message::new("connections.mcp.errors.changedMeanwhile"),
      Self::SecretStore { secret, source } => secret_store_message(secret, source),
      Self::SecretGone(_) => Message::new("errors.secretGone"),
      Self::SecretNotTaken(_) => Message::new("connections.mcp.errors.secretNotTaken"),
      Self::NotRestored { cause, .. } => match cause.field() {
        RefusedField::Secret(secret) => {
          Message::new("connections.mcp.errors.notRestored").with("secret", secret.to_string())
        }
        _ => Message::new("connections.mcp.errors.notRestoredOther"),
      },
    }
  }
}

/// The field of a save that a refusal is about, which the panel marks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefusedField {
  Program,
  /// The value of this secret.
  Secret(SecretName),
  /// No one field: the save as a whole.
  None,
}

impl McpSaveError {
  pub fn field(&self) -> RefusedField {
    match self {
      Self::Program(_) => RefusedField::Program,
      Self::SecretStore { secret, .. } => RefusedField::Secret(secret.clone()),
      Self::NotRestored { cause, .. } => cause.field(),
      _ => RefusedField::None,
    }
  }
}

/// Why the secret store did not keep the value of `secret`, in words that fit after *Couldn't save …:*. The value is
/// never in them.
fn secret_store_message(secret: &SecretName, source: &VaultError) -> Message {
  let key = match source {
    VaultError::AccessDenied(_) => "connections.mcp.errors.secretStoreRefused",
    VaultError::TooLong { .. } => "connections.mcp.errors.secretTooLong",
    VaultError::Unsupported => "connections.mcp.errors.secretStoreUnsupported",
    VaultError::Failed(_) | VaultError::NotFound => "connections.mcp.errors.secretStoreFailed",
  };
  let message = Message::new(key).with("secret", secret.to_string());
  match source {
    VaultError::TooLong { max } => message.with("max", max.to_string()),
    _ => message,
  }
}

/// Why `problem` keeps a program from being stored, in words.
pub fn program_problem_message(problem: &ProgramProblem) -> Message {
  match problem {
    ProgramProblem::InWorktree => Message::new("connections.mcp.errors.programInWorktree"),
    ProgramProblem::NotFound(named) => {
      Message::new("connections.mcp.errors.programNotFound").with("program", named.clone())
    }
    ProgramProblem::NotLocal(named) => {
      Message::new("connections.mcp.errors.programNotLocal").with("program", named.clone())
    }
    ProgramProblem::InRepository(found) => {
      Message::new("connections.mcp.errors.programInRepository").with("program", found.display().to_string())
    }
    ProgramProblem::NotText(found) => {
      Message::new("connections.mcp.errors.programNotText").with("program", found.display().to_string())
    }
    ProgramProblem::Invalid(error) => crate::tools::form::program_message(error),
  }
}

/// Stores `draft` as a new server, or as server `id`'s new settings, keeping whether it is on, when what it widens
/// over the server as stored now is `allowed`. A program another than the stored one is stored as the fixed file it
/// is, looked up on this app's search path, outside `repo_root`.
pub fn store_in(
  repo_root: &Path,
  id: Option<ConnectionId>,
  draft: McpServerDraft,
  allowed: &Allowed,
) -> Result<ConnectionId, McpSaveError> {
  let mut db = ProjectDb::open(repo_root)?;
  let stored = match id {
    Some(id) => Some(db.mcp_server(id)?.ok_or(StoreError::ConnectionNotFound(id))?),
    None => None,
  };
  let reasons = widenings(stored.as_ref(), &draft);
  if !allowed.covers(&reasons) {
    return Err(McpSaveError::Changed(reasons));
  }
  let transport = fixed_transport(repo_root, stored.as_ref(), draft.transport)?;
  let Some(mut server) = stored else {
    let mut server = McpServer::new(
      ConnectionId::generate(),
      draft.name,
      transport,
      draft.environment,
      Timestamp::now(),
    )?;
    server.set_exempt_tools(draft.exempt_tools);
    db.create_mcp_server(&server)?;
    return Ok(server.id());
  };
  server.update(draft.name, transport, draft.environment)?;
  server.set_exempt_tools(draft.exempt_tools);
  db.save_mcp_server(&server)?;
  Ok(server.id())
}

/// [`store_in`] for the server `id`, and the new `values` of the secrets its variables take, in `vault`, as one save:
/// when the secret store does not keep a value the server is put back as it was (what an earlier value of `values`
/// replaced stays replaced: the store cannot say what it held). A value for a secret the server does not take, or the
/// project does not have, is refused before anything is stored. When the server cannot be put back either, the error
/// says both ([`McpSaveError::NotRestored`]).
pub fn store_with_values(
  repo_root: &Path,
  vault: &dyn SecretVault,
  (id, draft, allowed): (ConnectionId, McpServerDraft, &Allowed),
  values: &[(SecretId, SecretValue)],
) -> Result<(), McpSaveError> {
  let mut db = ProjectDb::open(repo_root)?;
  let before = db.mcp_server(id)?.ok_or(StoreError::ConnectionNotFound(id))?;
  let taken = taken_secrets(&draft);
  for (secret_id, _) in values {
    if !taken.contains(secret_id) {
      return Err(match db.secret(*secret_id)? {
        Some(_) => McpSaveError::SecretNotTaken(*secret_id),
        None => McpSaveError::SecretGone(*secret_id),
      });
    }
  }
  store_in(repo_root, Some(id), draft, allowed)?;
  let project = db.project().id();
  for (secret_id, value) in values {
    let name = db.secret(*secret_id)?.map(|found| found.name().clone());
    if let Err(failure) = secret::replace_value(&mut db, vault, project, *secret_id, value, Timestamp::now()) {
      let cause = match (failure, name) {
        (SecretError::Vault(source), Some(secret)) => McpSaveError::SecretStore { secret, source },
        (SecretError::Store(error) | SecretError::ValueLeftBehind { source: error, .. }, _) => error.into(),
        (SecretError::RotationNotRecorded { source, .. }, _) => source.into(),
        _ => McpSaveError::SecretGone(*secret_id),
      };
      return Err(match db.save_mcp_server(&before) {
        Ok(()) => cause,
        Err(restore) => {
          tracing::error!(%restore, %cause, "cannot put the MCP server back as it was");
          McpSaveError::NotRestored {
            cause: Box::new(cause),
            restore,
          }
        }
      });
    }
  }
  Ok(())
}

/// The secrets the saved server takes: its authorization's, or its variables'.
fn taken_secrets(draft: &McpServerDraft) -> Vec<SecretId> {
  match &draft.transport {
    McpTransport::Http(http) => http.authorization().secret().into_iter().collect(),
    McpTransport::Stdio(_) => draft.environment.secrets().collect(),
  }
}

/// `transport` with its program fixed, unless it is the program `stored` already has.
fn fixed_transport(
  repo_root: &Path,
  stored: Option<&McpServer>,
  transport: McpTransport,
) -> Result<McpTransport, McpSaveError> {
  let McpTransport::Stdio(stdio) = transport else {
    return Ok(transport);
  };
  let unchanged = matches!(stored.map(McpServer::transport), Some(McpTransport::Stdio(before)) if before.program() == stdio.program());
  if unchanged {
    return Ok(McpTransport::Stdio(stdio));
  }
  let path = orchester_exec::search_path::search_path();
  let program = fixed_program(stdio.program(), path.as_deref(), repo_root).map_err(McpSaveError::Program)?;
  let directory = stdio.working_directory().cloned();
  Ok(McpTransport::Stdio(StdioServer::new(
    program,
    stdio.args().to_vec(),
    directory,
  )))
}

/// [`store_in`] for the old app's dialog, which a person fills in and which asks nothing, its failure in words.
pub fn save_in(repo_root: &Path, id: Option<ConnectionId>, draft: McpServerDraft) -> Result<ConnectionId, Message> {
  store_in(repo_root, id, draft, &Allowed::Anything).map_err(|error| error.message())
}

/// Turns server `id` on or off for every profile that chooses it.
pub fn set_enabled_in(repo_root: &Path, id: ConnectionId, enabled: bool) -> Result<(), StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  let mut server = db.mcp_server(id)?.ok_or(StoreError::ConnectionNotFound(id))?;
  server.set_enabled(enabled);
  db.save_mcp_server(&server)
}

/// Removes server `id` once no agent profile chooses it ([`server_uses`]), checked and removed in one transaction. A
/// profile that does would lose the server without a person deciding which profile works without it, so the
/// profiles switch first.
pub fn remove_in(repo_root: &Path, id: ConnectionId) -> Result<(), McpSaveError> {
  let mut db = ProjectDb::open(repo_root)?;
  let users = db.delete_unused_mcp_server(id, &move |db| profiles_using(db, id))?;
  if users.is_empty() {
    Ok(())
  } else {
    Err(McpSaveError::InUse(users))
  }
}

/// A profile that gives its agents an MCP server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerUse {
  pub server: ConnectionId,
  pub profile: ResourceId,
  pub role: Arc<str>,
  /// The model of the version that names the server: the draft's when the draft names it.
  pub model: ModelId,
}

/// Every profile's servers: those its draft names and those its latest published version names, in profile
/// order, each once. A run may start from the published version while the draft is edited, so both count.
pub fn server_uses<S: ResourceStore>(store: &S) -> Result<Vec<ServerUse>, S::Error> {
  let mut uses: Vec<ServerUse> = Vec::new();
  for resource in store.resources(ResourceKind::AgentProfile)? {
    let draft = store
      .draft::<AgentProfile>(resource.id())?
      .map(|draft| draft.body().clone());
    let latest = store
      .latest_version::<AgentProfile>(resource.id())?
      .map(|version| version.body().clone());
    for body in draft.iter().chain(latest.iter()) {
      for server in body.mcp_servers().ids() {
        if !uses
          .iter()
          .any(|used| used.server == *server && used.profile == resource.id())
        {
          uses.push(ServerUse {
            server: *server,
            profile: resource.id(),
            role: Arc::from(body.role().as_str()),
            model: body.model().clone(),
          });
        }
      }
    }
  }
  Ok(uses)
}

/// The roles of the profiles that choose server `id` ([`server_uses`]).
pub fn profiles_using<S: ResourceStore>(store: &S, id: ConnectionId) -> Result<Vec<Arc<str>>, S::Error> {
  Ok(
    server_uses(store)?
      .into_iter()
      .filter(|used| used.server == id)
      .map(|used| used.role)
      .collect(),
  )
}
