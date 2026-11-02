//! Decision model connections: secret references only in drafts and read models. Check lists models,
//! without sending project content or making an inference request.

mod edit;
mod http;
pub use edit::{check_draft_in, save_with_key_in};
pub mod test;

use std::path::Path;

use orchester_core::{
  connection::{DecisionCheck, DecisionCheckResult, DecisionConnection, DecisionServerUrl},
  id::{DecisionConnectionId, SecretId},
  name::Name,
  resource::ModelId,
  secret::SecretVault,
  store::SecretStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

/// The caller's authority, supplied by the window or the tool handler, never tool arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Writer {
  Person,
  Automation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionDraft {
  pub id: Option<DecisionConnectionId>,
  pub name: Name,
  pub base_url: DecisionServerUrl,
  pub key: Option<SecretId>,
}

#[derive(Debug, Error)]
pub enum DecisionError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error("person_only")]
  PersonOnly,
  #[error("the decision connection changed while its check ran")]
  CheckStale,
  #[error("the decision connection changed while it was being edited")]
  EditStale,
  #[error("the decision connection's key is missing or archived")]
  KeyUnavailable,
  #[error("the model-list HTTP client could not be initialized")]
  ClientUnavailable,
}

impl DecisionError {
  /// A secret-safe explanation for the window and background activity.
  pub fn message(&self) -> crate::messages::Message {
    use crate::messages::{self, Message};
    match self {
      Self::Store(error) => messages::store_error(error),
      Self::PersonOnly => Message::new("connections.errors.decisionPersonOnly"),
      Self::CheckStale => Message::new("connections.errors.decisionCheckStale"),
      Self::EditStale => Message::new("connections.errors.decisionEditStale"),
      Self::KeyUnavailable => Message::new("connections.errors.decisionKeyUnavailable"),
      Self::ClientUnavailable => Message::new("connections.errors.decisionClientUnavailable"),
    }
  }
}

/// Adds or edits a decision connection. Even renaming a hosted connection is a person's write.
/// A changed server/key clears the previous server's models and checks; a rename keeps them.
pub fn save_in(root: &Path, draft: &DecisionDraft, writer: Writer) -> Result<DecisionConnectionId, DecisionError> {
  if writer == Writer::Automation && !draft.base_url.is_on_this_computer() {
    return Err(DecisionError::PersonOnly);
  }
  let mut db = ProjectDb::open(root)?;
  let mut connection = match draft.id {
    Some(id) => load(&db, id)?,
    None => DecisionConnection::new(
      DecisionConnectionId::generate(),
      draft.name.clone(),
      draft.base_url.clone(),
      draft.key,
    ),
  };
  if writer == Writer::Automation && !connection.is_on_this_computer() {
    return Err(DecisionError::PersonOnly);
  }
  if let Some(key) = draft.key {
    ensure_key(&db, key)?;
  }
  connection.rename(draft.name.clone());
  connection.set_server(draft.base_url.clone(), draft.key);
  if draft.id.is_some() {
    db.save_decision_connection(&connection)?;
  } else {
    db.create_decision_connection(&connection)?;
  }
  Ok(connection.id())
}

pub fn delete_in(root: &Path, id: DecisionConnectionId) -> Result<(), DecisionError> {
  ProjectDb::open(root)?.delete_decision_connection(id)?;
  Ok(())
}

/// A bodyless GET of `{base_url}/models`. Only the private HTTP boundary reads the key; the returned
/// connection contains its id only. `selected` is the model whose disappearance should be recorded.
/// `DecisionCheck::at` records when the answer arrived, including unsuccessful checks.
pub async fn check_in(
  root: &Path,
  vault: &dyn SecretVault,
  id: DecisionConnectionId,
  selected: Option<&ModelId>,
) -> Result<DecisionConnection, DecisionError> {
  let (tested, project, key_available) = {
    let db = ProjectDb::open(root)?;
    let tested = load(&db, id)?;
    let key_available = match tested.key().map(|key| ensure_key(&db, key)).transpose() {
      Ok(_) => true,
      Err(DecisionError::KeyUnavailable) => false,
      Err(error) => return Err(error),
    };
    (tested, db.project().id(), key_available)
  };
  let (result, models) = if key_available {
    http::check(project, vault, &tested).await?
  } else {
    (DecisionCheckResult::KeyRefused, None)
  };
  let check = DecisionCheck {
    at: Timestamp::now(),
    result,
  };
  record_checked_in(root, &tested, check, models.as_deref(), selected)
}

/// Keeps edits made while HTTP was in flight; never attaches an old server's answer to a new one.
pub fn record_checked_in(
  root: &Path,
  tested: &DecisionConnection,
  mut check: DecisionCheck,
  models: Option<&[ModelId]>,
  selected: Option<&ModelId>,
) -> Result<DecisionConnection, DecisionError> {
  let mut db = ProjectDb::open(root)?;
  loop {
    let current = load(&db, tested.id())?;
    if current.base_url() != tested.base_url() || current.key() != tested.key() {
      return Err(DecisionError::CheckStale);
    }
    let mut checked = current.clone();
    if let Some(models) = models {
      if check.result == DecisionCheckResult::Connected
        && let Some(model) = selected.filter(|model| !models.contains(model))
      {
        check.result = DecisionCheckResult::ModelGone { model: model.clone() };
      }
      checked.record_models(models.to_vec());
    }
    checked.record_check(check.clone());
    if db.save_decision_check_if_unchanged(&current, &checked)? {
      return Ok(checked);
    }
  }
}

fn ensure_key(db: &ProjectDb, key: SecretId) -> Result<(), DecisionError> {
  match db.secret(key)? {
    Some(secret) if !secret.is_archived() => Ok(()),
    _ => Err(DecisionError::KeyUnavailable),
  }
}

fn load(db: &ProjectDb, id: DecisionConnectionId) -> Result<DecisionConnection, DecisionError> {
  db.decision_connection(id)?
    .ok_or_else(|| StoreError::DecisionConnectionNotFound(id).into())
}
