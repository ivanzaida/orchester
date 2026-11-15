//! How a decision model server's settings are stored, in a project's database and in the application's alike: its
//! base URL, the id its key goes by (never the key), the models its last check listed, and that check and test.

use orchester_core::{
  connection::{DecisionCheck, DecisionConnection, DecisionServerUrl, DecisionTest},
  id::{DecisionConnectionId, SecretId},
  name::Name,
  resource::ModelId,
};
use serde::{Deserialize, Serialize};

use crate::{
  StoreError,
  rows::{decode_json, encode_json},
};

const WHAT: &str = "decision connection settings";
const SCHEMA: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Settings {
  base_url: DecisionServerUrl,
  key: Option<SecretId>,
  models: Vec<ModelId>,
  last_check: Option<DecisionCheck>,
  last_test: Option<DecisionTest>,
}

/// The settings of `connection` as its row keeps them.
pub(crate) fn encode(connection: &DecisionConnection) -> Result<String, StoreError> {
  let settings = Settings {
    base_url: connection.base_url().clone(),
    key: connection.key(),
    models: connection.models().to_vec(),
    last_check: connection.last_check().cloned(),
    last_test: connection.last_test().cloned(),
  };
  encode_json(WHAT, SCHEMA, &settings)
}

/// The connection `id` named `name` with the settings its row keeps as `settings`.
pub(crate) fn decode(id: DecisionConnectionId, name: Name, settings: &str) -> Result<DecisionConnection, StoreError> {
  let settings: Settings = decode_json(WHAT, SCHEMA, settings)?;
  let mut connection = DecisionConnection::new(id, name, settings.base_url, settings.key).with_models(settings.models);
  if let Some(check) = settings.last_check {
    connection.record_check(check);
  }
  if let Some(test) = settings.last_test {
    connection.record_test(test);
  }
  Ok(connection)
}
