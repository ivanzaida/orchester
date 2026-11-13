//! The person's API accounts (`docs/design/agent-accounts.md`, "Agents run on accounts"): each one's kind of agent,
//! name, model API and models, and the last check of it. A removed account keeps its row, marked removed, so
//! finished runs still name what they ran on. The key is never here: it is in the OS keychain under the account's
//! id.

use orchester_core::{
  account::{AccountCli, ApiAccount, ApiSetup},
  connection::{AgentProvider, BaseUrl, CheckResult, ConnectionCheck, EndpointPreset, NativeModels},
  id::ApiAccountId,
  name::Name,
  resource::ModelId,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use super::AppDb;
use crate::{
  StoreError,
  project_db::{parse_provider, provider_text},
  rows::{corrupt, decode_json, encode_json, parse},
};

pub(super) const MIGRATION: &str = r"
CREATE TABLE api_accounts (
  id TEXT PRIMARY KEY,
  agent TEXT NOT NULL CHECK (agent IN ('codex_cli', 'claude_code_cli', 'anthropic_api', 'openai_api')),
  name TEXT NOT NULL,
  settings TEXT NOT NULL,
  added_at INTEGER NOT NULL,
  removed_at INTEGER,
  checked_at INTEGER,
  last_check TEXT,
  CHECK ((checked_at IS NULL) = (last_check IS NULL))
) STRICT;

CREATE UNIQUE INDEX api_account_names ON api_accounts (agent, lower(name)) WHERE removed_at IS NULL;
";

const SETTINGS_SCHEMA: u32 = 1;
const CHECK_SCHEMA: u32 = 1;
const SELECT: &str = "SELECT id, agent, name, settings, added_at, checked_at, last_check FROM api_accounts";

/// How an account's setup is stored; the kind of agent is the row's `agent`.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum StoredSetup {
  Cli {
    base_url: BaseUrl,
    models: Vec<ModelId>,
  },
  Native {
    preset: EndpointPreset,
    base_url: BaseUrl,
    #[serde(default, skip_serializing_if = "NativeModels::is_empty")]
    models: NativeModels,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    provider_fallback: bool,
  },
}

impl AppDb {
  /// The accounts that are not removed, by kind of agent, then in the order they were added.
  pub fn api_accounts(&self) -> Result<Vec<ApiAccount>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "{SELECT} WHERE removed_at IS NULL ORDER BY agent, added_at, id"
    ))?;
    let rows = statement
      .query_map([], AccountRow::read)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(AccountRow::into_account).collect()
  }

  /// The account, removed or not: a finished run still names what it ran on.
  pub fn api_account(&self, id: ApiAccountId) -> Result<Option<ApiAccount>, StoreError> {
    self
      .connection
      .query_row(&format!("{SELECT} WHERE id = ?1"), [id.to_string()], AccountRow::read)
      .optional()?
      .map(AccountRow::into_account)
      .transpose()
  }

  /// Adds `account`. A name another account of the same kind of agent has is refused.
  pub fn add_api_account(&mut self, account: &ApiAccount) -> Result<(), StoreError> {
    let result = self.connection.execute(
      "INSERT INTO api_accounts (id, agent, name, settings, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
      params![
        account.id().to_string(),
        provider_text(account.agent()),
        account.name().as_str(),
        settings_text(account.setup())?,
        account.added_at().unix_millis(),
      ],
    );
    name_taken(result, account)?;
    Ok(())
  }

  /// Saves the account's name, setup and last check; its kind of agent never changes.
  pub fn save_api_account(&mut self, account: &ApiAccount) -> Result<(), StoreError> {
    let check = account
      .last_check()
      .map(|check| encode_json("API account check", CHECK_SCHEMA, &check.result))
      .transpose()?;
    let result = self.connection.execute(
      "UPDATE api_accounts SET name = ?2, settings = ?3, checked_at = ?4, last_check = ?5 \
       WHERE id = ?1 AND agent = ?6 AND removed_at IS NULL",
      params![
        account.id().to_string(),
        account.name().as_str(),
        settings_text(account.setup())?,
        account.last_check().map(|check| check.at.unix_millis()),
        check,
        provider_text(account.agent()),
      ],
    );
    if name_taken(result, account)? == 0 {
      return Err(StoreError::ApiAccountNotFound(account.id()));
    }
    Ok(())
  }

  /// Marks the account removed: it leaves the list and its name is free again, and runs that used it still name it.
  pub fn remove_api_account(&mut self, id: ApiAccountId, now: Timestamp) -> Result<(), StoreError> {
    let changed = self.connection.execute(
      "UPDATE api_accounts SET removed_at = ?2 WHERE id = ?1 AND removed_at IS NULL",
      params![id.to_string(), now.unix_millis()],
    )?;
    if changed == 0 {
      return Err(StoreError::ApiAccountNotFound(id));
    }
    Ok(())
  }
}

/// The rows a write changed, or that the table refused it for a taken name.
fn name_taken(result: rusqlite::Result<usize>, account: &ApiAccount) -> Result<usize, StoreError> {
  match result {
    Err(rusqlite::Error::SqliteFailure(failure, _)) if failure.code == rusqlite::ErrorCode::ConstraintViolation => {
      Err(StoreError::DuplicateAccountName(account.name().as_str().to_owned()))
    }
    other => Ok(other?),
  }
}

fn settings_text(setup: &ApiSetup) -> Result<String, StoreError> {
  let stored = match setup.clone() {
    ApiSetup::Cli { base_url, models, .. } => StoredSetup::Cli { base_url, models },
    ApiSetup::Native {
      preset,
      base_url,
      models,
      provider_fallback,
      ..
    } => StoredSetup::Native {
      preset,
      base_url,
      models,
      provider_fallback,
    },
  };
  encode_json("API account settings", SETTINGS_SCHEMA, &stored)
}

/// An `api_accounts` row as stored, before its values are checked.
struct AccountRow {
  id: String,
  agent: String,
  name: String,
  settings: String,
  added_at: i64,
  checked_at: Option<i64>,
  last_check: Option<String>,
}

impl AccountRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      agent: row.get(1)?,
      name: row.get(2)?,
      settings: row.get(3)?,
      added_at: row.get(4)?,
      checked_at: row.get(5)?,
      last_check: row.get(6)?,
    })
  }

  fn into_account(self) -> Result<ApiAccount, StoreError> {
    let setup = setup(parse_provider(&self.agent)?, &self.settings)?;
    let last_check = match (self.checked_at, self.last_check) {
      (Some(at), Some(result)) => Some(ConnectionCheck {
        at: Timestamp::from_unix_millis(at),
        result: decode_json::<CheckResult>("API account check", CHECK_SCHEMA, &result)?,
      }),
      _ => None,
    };
    ApiAccount::from_stored(
      parse("API account id", &self.id)?,
      Name::new(&self.name).map_err(|error| corrupt("API account name", error))?,
      setup,
      Timestamp::from_unix_millis(self.added_at),
      last_check,
    )
    .map_err(|error| corrupt("API account", error))
  }
}

/// The setup stored as `settings` for an account of `agent`: a CLI's for an agent CLI, a model API's for one of
/// Orchester's own agents.
fn setup(agent: AgentProvider, settings: &str) -> Result<ApiSetup, StoreError> {
  let stored: StoredSetup = decode_json("API account settings", SETTINGS_SCHEMA, settings)?;
  match (stored, AccountCli::of(agent)) {
    (StoredSetup::Cli { base_url, models }, Some(cli)) => ApiSetup::cli(cli, base_url, models),
    (
      StoredSetup::Native {
        preset,
        base_url,
        models,
        provider_fallback,
      },
      None,
    ) => ApiSetup::native(agent, preset, base_url, models, provider_fallback),
    (StoredSetup::Cli { .. }, None) | (StoredSetup::Native { .. }, Some(_)) => {
      return Err(corrupt("API account settings", "not the settings of its kind of agent"));
    }
  }
  .map_err(|error| corrupt("API account settings", error))
}
