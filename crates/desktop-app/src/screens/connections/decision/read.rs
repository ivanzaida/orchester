//! Reads the selected server and the reviewer that uses it, without reading any secret value.
use std::path::PathBuf;

use lurq::app::ctx::{Ctx, FutureState};
use orchester_core::{
  connection::DecisionConnection,
  id::DecisionConnectionId,
  run::review::{ReviewerChoice, ReviewerConnectionId},
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  background,
  messages::{self, Message},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Read {
  pub connection: Option<DecisionConnection>,
  pub reviewer: Option<ReviewerChoice>,
  pub key_name: Option<String>,
  pub listed: orchester_workspace::connections::data::ProjectConnections,
}

impl lurq::app::component::DevtoolsInspectable for Read {}

pub(super) fn load(
  ctx: &mut Ctx,
  root: PathBuf,
  id: DecisionConnectionId,
  revision: u64,
) -> FutureState<Read, Message> {
  ctx
    .future((root, id, revision), |(root, id, _)| async move {
      background::run(move || {
        let db = ProjectDb::open(&root)?;
        let connection = db.decision_connection(id)?;
        let reviewer = db
          .auto_mode_settings()?
          .reviewer
          .filter(|choice| choice.connection == ReviewerConnectionId::Decision { decision: id });
        let key_name = connection
          .as_ref()
          .and_then(DecisionConnection::key)
          .map(|id| orchester_core::store::SecretStore::secret(&db, id))
          .transpose()?
          .flatten()
          .map(|secret| secret.name().as_str().to_owned());
        Ok::<_, orchester_store::StoreError>(Read {
          connection,
          reviewer,
          key_name,
          listed: orchester_workspace::connections::data::ProjectConnections::load(&db)?,
        })
      })
      .await?
      .map_err(|error| messages::store_error(&error))
    })
    .state()
    .get()
}
