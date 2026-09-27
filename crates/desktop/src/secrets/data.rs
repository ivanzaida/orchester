//! Reading the project's secrets with the tools that use them, and adding, replacing and
//! archiving secrets through the project database and the keychain. Every function blocks on
//! SQLite or on the keychain; the UI calls them off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::{ResourceId, SecretId},
  resource::{ResourceKind, Tool},
  secret::{self, NewSecret, Secret, SecretError, SecretValue, SecretVault},
  store::{ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

/// Why a secret could not be added, replaced or archived.
pub type SecretFailure = SecretError<StoreError>;

/// A tool whose current content refers to a secret, for the read-only "Used by" list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolUse {
  pub secret: SecretId,
  pub tool: ResourceId,
  pub name: Arc<str>,
}

/// The project's secrets that are not archived, and the tools that use them, read together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectSecrets {
  pub secrets: Arc<Vec<Secret>>,
  pub uses: Arc<Vec<ToolUse>>,
}

impl lurq::app::component::DevtoolsInspectable for ProjectSecrets {}

impl ProjectSecrets {
  pub fn load<S>(store: &S) -> Result<Self, StoreError>
  where
    S: SecretStore<Error = StoreError> + ResourceStore<Error = StoreError>,
  {
    let secrets = store.secrets()?;
    let tools = current_tools(store)?;
    let uses = secrets
      .iter()
      .flat_map(|secret| {
        tools
          .iter()
          .filter(|current| secret::tool_uses(&current.body, secret.id()))
          .map(|current| ToolUse {
            secret: secret.id(),
            tool: current.id,
            name: current.name.clone(),
          })
      })
      .collect();
    Ok(Self {
      secrets: Arc::new(secrets),
      uses: Arc::new(uses),
    })
  }

  pub fn load_in(repo_root: &Path) -> Result<Self, StoreError> {
    Self::load(&ProjectDb::open(repo_root)?)
  }

  pub fn secret(&self, id: SecretId) -> Option<&Secret> {
    self.secrets.iter().find(|secret| secret.id() == id)
  }

  /// Tools that use secret `id`, in tool order.
  pub fn used_by(&self, id: SecretId) -> impl Iterator<Item = &ToolUse> {
    self.uses.iter().filter(move |used| used.secret == id)
  }
}

/// A tool of the project with its current content: its draft if it has one, else its latest
/// published version.
struct CurrentTool {
  id: ResourceId,
  name: Arc<str>,
  body: Tool,
}

/// Each tool of the project with its current content. A tool that has neither a draft nor a
/// version refers to nothing.
fn current_tools<S: ResourceStore>(store: &S) -> Result<Vec<CurrentTool>, S::Error> {
  let mut tools = Vec::new();
  for resource in store.resources(ResourceKind::Tool)? {
    let body = match store.draft::<Tool>(resource.id())? {
      Some(draft) => Some(draft.body().clone()),
      None => store
        .latest_version::<Tool>(resource.id())?
        .map(|version| version.body().clone()),
    };
    if let Some(body) = body {
      tools.push(CurrentTool {
        id: resource.id(),
        name: resource.name().as_str().into(),
        body,
      });
    }
  }
  Ok(tools)
}

/// Adds a secret to the project at `repo_root`, its value into `vault`.
pub fn add_in(
  repo_root: &Path,
  vault: &dyn SecretVault,
  new: NewSecret,
  value: &SecretValue,
) -> Result<SecretId, SecretFailure> {
  let mut db = ProjectDb::open(repo_root).map_err(SecretError::Store)?;
  let project = db.project().id();
  secret::add(&mut db, vault, project, new, value, Timestamp::now()).map(|secret| secret.id())
}

/// Replaces the value of secret `id` of the project at `repo_root`.
pub fn replace_in(
  repo_root: &Path,
  vault: &dyn SecretVault,
  id: SecretId,
  value: &SecretValue,
) -> Result<(), SecretFailure> {
  let mut db = ProjectDb::open(repo_root).map_err(SecretError::Store)?;
  let project = db.project().id();
  secret::replace_value(&mut db, vault, project, id, value, Timestamp::now()).map(|_| ())
}

/// Archives secret `id` of the project at `repo_root`, removing its value from `vault`.
pub fn archive_in(repo_root: &Path, vault: &dyn SecretVault, id: SecretId) -> Result<(), SecretFailure> {
  let mut db = ProjectDb::open(repo_root).map_err(SecretError::Store)?;
  let project = db.project().id();
  secret::archive(&mut db, vault, project, id, Timestamp::now()).map(|_| ())
}
