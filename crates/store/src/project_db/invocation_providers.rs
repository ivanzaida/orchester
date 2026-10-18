//! The API provider each agent CLI invocation on one started on (`invocation_providers`): its base URL and the project
//! secret holding its key, written once by the runner before the CLI starts (`docs/design/api-providers.md`). A later
//! invocation of the step continues the session only on the same provider.

use orchester_core::{
  connection::BaseUrl,
  id::{InvocationId, SecretId},
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

impl ProjectDb {
  /// Records that `invocation` starts on the API provider at `base_url` with the key in secret `key`.
  pub fn save_invocation_provider(
    &mut self,
    invocation: InvocationId,
    base_url: &BaseUrl,
    key: SecretId,
  ) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO invocation_providers (invocation_id, base_url, key_secret_id) VALUES (?1, ?2, ?3)",
      params![invocation.to_string(), base_url.as_str(), key.to_string()],
    )?;
    Ok(())
  }

  /// The API provider `invocation` started on, if it started on one.
  pub fn invocation_provider(&self, invocation: InvocationId) -> Result<Option<(BaseUrl, SecretId)>, StoreError> {
    let row: Option<(String, String)> = self
      .connection
      .query_row(
        "SELECT base_url, key_secret_id FROM invocation_providers WHERE invocation_id = ?1",
        [invocation.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    row
      .map(|(base_url, key)| {
        let base_url = BaseUrl::new(&base_url).map_err(|error| corrupt("invocation provider", error))?;
        Ok((base_url, parse("secret id", &key)?))
      })
      .transpose()
  }
}
