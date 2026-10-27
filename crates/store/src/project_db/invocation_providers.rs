//! The API provider each agent CLI invocation on one started on (`invocation_providers`): its base URL and the project
//! secret holding its key, written once by the runner before the CLI starts (`docs/design/api-providers.md`). A later
//! invocation of the step continues the session only on the same provider. What the invocations on a provider spent
//! from a time on is read by the provider, as the Orchestrator's runs-on picker shows it.

use orchester_core::{
  connection::{BaseUrl, stream::AgentEvent},
  id::{InvocationId, SecretId},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::{ProjectDb, activity::EVENT_SCHEMA};
use crate::{
  StoreError,
  rows::{corrupt, decode_json, parse},
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

  /// The tokens that count toward a budget of the turns completed from `since` on by the invocations that started on
  /// the API provider at `base_url` with the key in secret `key`, as their CLIs reported them.
  pub fn api_provider_tokens_since(
    &self,
    (base_url, key): (&BaseUrl, SecretId),
    since: Timestamp,
  ) -> Result<u64, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT events.payload FROM agent_events AS events \
       JOIN invocation_providers AS providers ON providers.invocation_id = events.invocation_id \
       WHERE events.kind = 'turn_completed' AND events.at >= ?1 \
         AND providers.base_url = ?2 AND providers.key_secret_id = ?3",
    )?;
    let payloads = statement
      .query_map(
        params![since.unix_millis(), base_url.as_str(), key.to_string()],
        |row| row.get::<_, String>(0),
      )?
      .collect::<Result<Vec<_>, _>>()?;
    payloads.iter().try_fold(0_u64, |sum, payload| {
      match decode_json("agent event", EVENT_SCHEMA, payload)? {
        AgentEvent::TurnCompleted { usage } => Ok(sum.saturating_add(usage.counted())),
        other => Err(corrupt(
          "agent event",
          format!("a `turn_completed` row holds `{}`", other.name()),
        )),
      }
    })
  }
}
