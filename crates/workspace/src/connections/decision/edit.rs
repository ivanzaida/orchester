//! Form operations reuse the connection writer and secret lifecycle, without persisting draft checks.
use std::path::Path;

use orchester_core::{
  connection::{DecisionCheck, DecisionConnection},
  secret::{SecretValue, SecretVault, VaultKey},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{DecisionDraft, DecisionError, Writer, http, load, save_in};
use crate::{
  connections::native::{KeySecret, with_key},
  messages::{self, Message},
};

/// Saves a form's optional key, rolling a newly created secret back if the connection cannot be saved.
/// An editor supplies its originally displayed connection to reject concurrent changes atomically.
pub fn save_with_key_in(
  root: &Path,
  vault: &dyn SecretVault,
  draft: &DecisionDraft,
  (key, value): (Option<&KeySecret>, Option<SecretValue>),
  writer: Writer,
  expected: Option<&DecisionConnection>,
) -> Result<orchester_core::id::DecisionConnectionId, Message> {
  let mut db = ProjectDb::open(root).map_err(|e| messages::store_error(&e))?;
  let stored = draft.id.map(|id| load(&db, id)).transpose().map_err(|e| e.message())?;
  if writer == Writer::Automation
    && (!draft.base_url.is_on_this_computer() || stored.as_ref().is_some_and(|c| !c.is_on_this_computer()))
  {
    return Err(DecisionError::PersonOnly.message());
  }
  if expected.is_some_and(|expected| stored.as_ref() != Some(expected)) {
    return Err(DecisionError::EditStale.message());
  }
  let save = |db: &mut ProjectDb, key| {
    let mut draft = draft.clone();
    draft.key = key;
    if let Some(expected) = expected {
      save_edit(db, &draft, expected).map_err(|e| e.message())
    } else {
      save_in(root, &draft, writer).map_err(|e| e.message())
    }
  };
  let Some(key) = key else {
    return save(&mut db, None);
  };
  with_key(&mut db, vault, (key, value), |db, key| save(db, Some(key)))
}

fn save_edit(
  db: &mut ProjectDb,
  draft: &DecisionDraft,
  expected: &DecisionConnection,
) -> Result<orchester_core::id::DecisionConnectionId, DecisionError> {
  if draft.id != Some(expected.id()) {
    return Err(DecisionError::EditStale);
  }
  if let Some(key) = draft.key {
    super::ensure_key(db, key)?;
  }
  let mut edited = expected.clone();
  edited.rename(draft.name.clone());
  edited.set_server(draft.base_url.clone(), draft.key);
  if !db.save_decision_connection_if_unchanged(expected, &edited)? {
    return Err(DecisionError::EditStale);
  }
  Ok(edited.id())
}

/// Lists models for unsaved fields. Neither a draft connection nor a new key is stored.
pub async fn check_draft_in(
  root: &Path,
  vault: &dyn SecretVault,
  draft: &DecisionDraft,
  key: Option<&KeySecret>,
  value: Option<SecretValue>,
  writer: Writer,
) -> Result<DecisionConnection, Message> {
  let db = ProjectDb::open(root).map_err(|e| messages::store_error(&e))?;
  if writer == Writer::Automation && !draft.base_url.is_on_this_computer() {
    let stored = draft.id.map(|id| load(&db, id)).transpose().map_err(|e| e.message())?;
    let same = stored.is_some_and(|stored| {
      stored.base_url() == &draft.base_url && key == stored.key().map(KeySecret::Existing).as_ref()
    });
    if !same {
      return Err(DecisionError::PersonOnly.message());
    }
  }
  let value = match key {
    Some(KeySecret::Existing(id)) => {
      super::ensure_key(&db, *id).map_err(|e| e.message())?;
      Some(
        vault
          .read(&VaultKey::new(db.project().id(), *id))
          .map_err(|_| DecisionError::KeyUnavailable.message())?,
      )
    }
    Some(KeySecret::New(_)) => Some(value.ok_or_else(|| Message::new("connections.native.key.valueRequired"))?),
    None => None,
  };
  let (result, models) = http::check_with_key(&draft.base_url, value.as_ref())
    .await
    .map_err(|e| e.message())?;
  let mut connection = DecisionConnection::new(
    draft
      .id
      .unwrap_or_else(orchester_core::id::DecisionConnectionId::generate),
    draft.name.clone(),
    draft.base_url.clone(),
    draft.key,
  );
  if let Some(models) = models {
    connection.record_models(models);
  }
  connection.record_check(DecisionCheck {
    at: Timestamp::now(),
    result,
  });
  Ok(connection)
}
