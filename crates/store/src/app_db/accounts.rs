//! The person's agent CLI accounts (`docs/design/agent-accounts.md`): each one's CLI, name, folder and last check.
//! Each CLI's Default account is a row from the start, under its fixed id, with no folder (the CLI's own) and no
//! name until the person renames it. A removed account keeps its row, marked removed, so finished runs still name the
//! account they ran on. No credential is ever stored: a check holds only who the CLI says is signed in.

use std::path::PathBuf;

use orchester_core::{
  account::{AccountCheck, AccountCli, AccountFolder, AgentAccount},
  id::AgentAccountId,
  name::Name,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Row, params};

use super::AppDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse, path_text},
};

pub(super) const MIGRATION: &str = r"
CREATE TABLE agent_accounts (
  id TEXT PRIMARY KEY,
  cli TEXT NOT NULL CHECK (cli IN ('claude_code', 'codex')),
  name TEXT,
  folder TEXT,
  folder_kind TEXT NOT NULL CHECK (folder_kind IN ('own', 'created', 'brought')),
  added_at INTEGER NOT NULL,
  removed_at INTEGER,
  moved_to TEXT,
  checked_at INTEGER,
  last_check TEXT,
  CHECK ((folder IS NULL) = (folder_kind = 'own')),
  CHECK ((checked_at IS NULL) = (last_check IS NULL)),
  CHECK (removed_at IS NOT NULL OR moved_to IS NULL)
) STRICT;

CREATE UNIQUE INDEX agent_account_names ON agent_accounts (cli, lower(name))
  WHERE removed_at IS NULL AND name IS NOT NULL;
CREATE UNIQUE INDEX agent_account_folders ON agent_accounts (lower(folder))
  WHERE removed_at IS NULL AND folder IS NOT NULL;

INSERT INTO agent_accounts (id, cli, name, folder, folder_kind, added_at) VALUES
  ('00000000-0000-7000-8000-000000000001', 'claude_code', NULL, NULL, 'own', 0),
  ('00000000-0000-7000-8000-000000000002', 'codex', NULL, NULL, 'own', 0);
";

const CHECK_SCHEMA: u32 = 1;
const SELECT: &str = "SELECT id, cli, name, folder, folder_kind, added_at, checked_at, last_check FROM agent_accounts";

impl AppDb {
  /// The accounts, by CLI, Default first, then in the order they were added.
  pub fn agent_accounts(&self) -> Result<Vec<AgentAccount>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "{SELECT} WHERE removed_at IS NULL ORDER BY cli, folder_kind <> 'own', added_at, id"
    ))?;
    let rows = statement
      .query_map([], AccountRow::read)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(AccountRow::into_account).collect()
  }

  /// The account, removed or not: a finished run still names the account it ran on.
  pub fn agent_account(&self, id: AgentAccountId) -> Result<Option<AgentAccount>, StoreError> {
    self
      .connection
      .query_row(&format!("{SELECT} WHERE id = ?1"), [id.to_string()], AccountRow::read)
      .optional()?
      .map(AccountRow::into_account)
      .transpose()
  }

  /// Adds `account`. A name another account of the CLI has, or a folder another account has, is refused.
  pub fn add_agent_account(&mut self, account: &AgentAccount) -> Result<(), StoreError> {
    let (folder, kind) = folder_columns(account.folder())?;
    let result = self.connection.execute(
      "INSERT INTO agent_accounts (id, cli, name, folder, folder_kind, added_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
      params![
        account.id().to_string(),
        account.cli().as_str(),
        account.name().map(Name::as_str),
        folder,
        kind,
        account.added_at().unix_millis(),
      ],
    );
    constraint_errors(result, account)?;
    Ok(())
  }

  /// Saves the account's name and last check; its CLI and folder never change.
  pub fn save_agent_account(&mut self, account: &AgentAccount) -> Result<(), StoreError> {
    let check = account
      .last_check()
      .map(|check| encode_json("account check", CHECK_SCHEMA, &check.result))
      .transpose()?;
    let result = self.connection.execute(
      "UPDATE agent_accounts SET name = ?2, checked_at = ?3, last_check = ?4 WHERE id = ?1 AND removed_at IS NULL",
      params![
        account.id().to_string(),
        account.name().map(Name::as_str),
        account.last_check().map(|check| check.at.unix_millis()),
        check,
      ],
    );
    if constraint_errors(result, account)? == 0 {
      return Err(StoreError::AgentAccountNotFound(account.id()));
    }
    Ok(())
  }

  /// Marks the account removed: it leaves the list, its name and folder are free again, and runs that used it still
  /// name it. `moved_to` is where its connections went (`None`: no account), for a project that could not be reached
  /// then. Default is never removed.
  pub fn remove_agent_account(
    &mut self,
    id: AgentAccountId,
    moved_to: Option<AgentAccountId>,
    now: Timestamp,
  ) -> Result<(), StoreError> {
    let changed = self.connection.execute(
      "UPDATE agent_accounts SET removed_at = ?2, moved_to = ?3 \
       WHERE id = ?1 AND removed_at IS NULL AND folder_kind <> 'own'",
      params![
        id.to_string(),
        now.unix_millis(),
        moved_to.map(|account| account.to_string())
      ],
    )?;
    if changed == 0 {
      return Err(StoreError::AgentAccountNotFound(id));
    }
    Ok(())
  }

  /// The removed accounts and where each one's connections went (`None`: no account).
  pub fn removed_agent_accounts(&self) -> Result<Vec<(AgentAccountId, Option<AgentAccountId>)>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT id, moved_to FROM agent_accounts WHERE removed_at IS NOT NULL")?;
    let rows = statement
      .query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(id, moved_to)| {
        Ok((
          parse("account id", &id)?,
          moved_to.map(|moved| parse("account id", &moved)).transpose()?,
        ))
      })
      .collect()
  }
}

/// The rows a write changed, or why the table refused it: a taken name or folder.
fn constraint_errors(result: rusqlite::Result<usize>, account: &AgentAccount) -> Result<usize, StoreError> {
  match result {
    Err(rusqlite::Error::SqliteFailure(failure, Some(message)))
      if failure.code == rusqlite::ErrorCode::ConstraintViolation =>
    {
      if message.contains("folder") {
        Err(StoreError::AccountFolderTaken)
      } else {
        Err(StoreError::DuplicateAccountName(
          account.name().map(|name| name.as_str().to_owned()).unwrap_or_default(),
        ))
      }
    }
    other => Ok(other?),
  }
}

fn folder_columns(folder: &AccountFolder) -> Result<(Option<&str>, &'static str), StoreError> {
  Ok(match folder {
    AccountFolder::Own => (None, "own"),
    AccountFolder::Created(path) => (Some(path_text("account folder", path)?), "created"),
    AccountFolder::Brought(path) => (Some(path_text("account folder", path)?), "brought"),
  })
}

/// An `agent_accounts` row as stored, before its values are checked.
struct AccountRow {
  id: String,
  cli: String,
  name: Option<String>,
  folder: Option<String>,
  folder_kind: String,
  added_at: i64,
  checked_at: Option<i64>,
  last_check: Option<String>,
}

impl AccountRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      cli: row.get(1)?,
      name: row.get(2)?,
      folder: row.get(3)?,
      folder_kind: row.get(4)?,
      added_at: row.get(5)?,
      checked_at: row.get(6)?,
      last_check: row.get(7)?,
    })
  }

  fn into_account(self) -> Result<AgentAccount, StoreError> {
    let cli = AccountCli::parse(&self.cli).ok_or_else(|| corrupt("account CLI", &self.cli))?;
    let folder = match (self.folder_kind.as_str(), self.folder) {
      ("own", None) => AccountFolder::Own,
      ("created", Some(path)) => AccountFolder::Created(PathBuf::from(path)),
      ("brought", Some(path)) => AccountFolder::Brought(PathBuf::from(path)),
      (kind, _) => return Err(corrupt("account folder", kind)),
    };
    let name = self
      .name
      .map(|name| Name::new(&name).map_err(|error| corrupt("account name", error)))
      .transpose()?;
    let last_check = match (self.checked_at, self.last_check) {
      (Some(at), Some(result)) => Some(AccountCheck {
        at: Timestamp::from_unix_millis(at),
        result: decode_json("account check", CHECK_SCHEMA, &result)?,
      }),
      _ => None,
    };
    AgentAccount::from_stored(
      parse("account id", &self.id)?,
      cli,
      name,
      folder,
      Timestamp::from_unix_millis(self.added_at),
      last_check,
    )
    .map_err(|error| corrupt("account", error))
  }
}
