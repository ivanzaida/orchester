//! The application database next to the executable: known projects, the session log used to
//! detect that the previous run of Orchester ended abnormally, whose each project is ([`owners`]), the state of the
//! window (`ui_state`) that outlives a session, such as its tabs, the shared servers ([`servers`]) and whose each one
//! is ([`server_owners`]), the application's own settings ([`settings`]), among them the Usage dashboard's warning
//! threshold, and its updates' ([`updates`]), the person's agent CLI accounts ([`accounts`]) and the plan limits read
//! of them outside runs ([`plan_reads`]).

mod accounts;
mod cloud;
mod cloud_ended;
mod cloud_joins;
mod owners;
mod plan_reads;
mod server_owners;
mod servers;
mod settings;
mod updates;

use std::path::{Path, PathBuf};

use orchester_core::{id::ProjectId, name::Name, time::Timestamp};
use rusqlite::{Connection, OptionalExtension, params};

pub use self::{
  cloud::{StoredCloudAccount, StoredCloudAddress},
  cloud_ended::{CloudSignInEnd, EndedCloudSignIn},
  cloud_joins::AcceptedInvite,
  servers::{JoinedWith, ServerAccount, StoredServer},
};
use crate::{
  StoreError, migrate,
  rows::{corrupt, parse},
};

const MIGRATIONS: &[&str] = &[
  r"
CREATE TABLE projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  repo_root TEXT NOT NULL UNIQUE,
  added_at INTEGER NOT NULL,
  last_opened_at INTEGER NOT NULL
) STRICT;

CREATE TABLE sessions (
  id INTEGER PRIMARY KEY,
  pid INTEGER NOT NULL,
  started_at INTEGER NOT NULL,
  ended_at INTEGER,
  end_kind TEXT
) STRICT;
",
  r"
CREATE TABLE ui_state (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT;
",
  servers::MIGRATION,
  settings::MIGRATION,
  settings::THRESHOLD_MIGRATION,
  settings::APPEARANCE_MIGRATION,
  settings::AUTOMATION_PERMISSIONS_MIGRATION,
  settings::APP_MCP_MIGRATION,
  accounts::MIGRATION,
  servers::PIN_MIGRATION,
  settings::ORCHESTRATOR_PUBLISHES_MIGRATION,
  cloud::MIGRATION,
  cloud_joins::MIGRATION,
  cloud_joins::ASKS_MIGRATION,
  cloud_ended::MIGRATION,
  owners::MIGRATION,
  server_owners::MIGRATION,
  cloud_ended::TWO_FACTOR_MIGRATION,
  cloud_joins::NEEDS_CODE_MIGRATION,
  settings::ORCHESTRATOR_ANSWERS_MERGES_MIGRATION,
  settings::ORCHESTRATOR_CONTINUES_MIGRATION,
  plan_reads::MIGRATION,
  settings::NOTIFICATIONS_MIGRATION,
  settings::DAILY_REPORT_MIGRATION,
  settings::STORY_POINTS_MIGRATION,
  settings::ORCHESTRATOR_MERGES_FINISHED_MIGRATION,
  settings::ORCHESTRATOR_WRITES_GIT_MIGRATION,
  cloud::API_ADDRESS_MIGRATION,
  settings::ERROR_REPORTS_MIGRATION,
  updates::MIGRATION,
];

/// Identifies the running session in the session log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionId(i64);

/// How a session ended when the application closed normally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionEnd {
  /// No runs were active.
  Clean,
  /// The user waited for agents to reach checkpoints; the runs are paused.
  RunsPaused,
  /// The user chose Quit now; active runs were terminated without a checkpoint.
  QuitNow,
}

/// What the previous session left behind, reported when a new session starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviousSession {
  None,
  Ended(SessionEnd),
  /// The previous session never recorded its end: the process crashed or was killed.
  Crashed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredProject {
  pub id: ProjectId,
  pub name: Name,
  pub repo_root: PathBuf,
  pub last_opened_at: Timestamp,
}

impl RegisteredProject {
  /// Whether the project's folder is gone from where it was registered (moved, renamed or
  /// deleted). A folder that cannot be checked, e.g. for lack of permission, is not reported
  /// missing: opening the project reports why it cannot be read.
  pub fn folder_missing(&self) -> bool {
    matches!(self.repo_root.try_exists(), Ok(false))
  }
}

pub struct AppDb {
  connection: Connection,
}

impl AppDb {
  pub fn open(path: &Path) -> Result<Self, StoreError> {
    Ok(Self {
      connection: migrate::open(path, MIGRATIONS)?,
    })
  }

  /// The database's file as SQLite opened it; `None` for a database only in memory.
  pub fn path(&self) -> Option<PathBuf> {
    self
      .connection
      .path()
      .filter(|path| !path.is_empty())
      .map(PathBuf::from)
  }

  /// Creates the database at `path` at schema `version`, with only the first `version`
  /// migrations applied, and returns it empty: a migration test fills it with the rows that
  /// version wrote, then [`AppDb::open`] applies the rest.
  #[cfg(feature = "test-support")]
  pub fn create_at_version(path: &Path, version: u32) -> Result<Connection, StoreError> {
    let Some(migrations) = MIGRATIONS.get(..version as usize) else {
      let supported = u32::try_from(MIGRATIONS.len()).expect("fewer than 2^32 migrations");
      return Err(StoreError::SchemaTooNew {
        found: version,
        supported,
      });
    };
    migrate::open(path, migrations)
  }

  /// Records a new session and reports how the previous one ended. A previous session with
  /// no recorded end is marked crashed so it is reported once.
  pub fn start_session(&mut self, pid: u32, now: Timestamp) -> Result<(SessionId, PreviousSession), StoreError> {
    let transaction = self.connection.transaction()?;
    let previous: Option<(i64, Option<String>)> = transaction
      .query_row(
        "SELECT id, end_kind FROM sessions ORDER BY id DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    let previous = match previous {
      None => PreviousSession::None,
      Some((id, None)) => {
        transaction.execute("UPDATE sessions SET end_kind = 'crashed' WHERE id = ?1", [id])?;
        PreviousSession::Crashed
      }
      Some((_, Some(kind))) => parse_end(&kind)?,
    };
    transaction.execute(
      "INSERT INTO sessions (pid, started_at) VALUES (?1, ?2)",
      params![pid, now.unix_millis()],
    )?;
    let session = SessionId(transaction.last_insert_rowid());
    transaction.commit()?;
    Ok((session, previous))
  }

  /// When the session before `session` started; `None` for the first session.
  pub fn session_started_before(&self, session: SessionId) -> Result<Option<Timestamp>, StoreError> {
    let started: Option<i64> = self
      .connection
      .query_row(
        "SELECT started_at FROM sessions WHERE id < ?1 ORDER BY id DESC LIMIT 1",
        [session.0],
        |row| row.get(0),
      )
      .optional()?;
    Ok(started.map(Timestamp::from_unix_millis))
  }

  /// The window state stored under `key`; `None` when nothing was stored.
  pub fn ui_state(&self, key: &str) -> Result<Option<String>, StoreError> {
    Ok(
      self
        .connection
        .query_row("SELECT value FROM ui_state WHERE key = ?1", [key], |row| row.get(0))
        .optional()?,
    )
  }

  /// Stores `value` under `key`, in place of what was stored there.
  pub fn set_ui_state(&mut self, key: &str, value: &str) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO ui_state (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
      params![key, value],
    )?;
    Ok(())
  }

  /// Removes what was stored under `key`; nothing happens when nothing was.
  pub fn remove_ui_state(&mut self, key: &str) -> Result<(), StoreError> {
    self.connection.execute("DELETE FROM ui_state WHERE key = ?1", [key])?;
    Ok(())
  }

  /// The keys window state is stored under that start with `prefix`, in order.
  pub fn ui_state_keys(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT key FROM ui_state WHERE substr(key, 1, length(?1)) = ?1 ORDER BY key")?;
    let keys = statement
      .query_map([prefix], |row| row.get(0))?
      .collect::<Result<Vec<String>, _>>()?;
    Ok(keys)
  }

  pub fn end_session(&mut self, session: SessionId, end: SessionEnd, now: Timestamp) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE sessions SET ended_at = ?2, end_kind = ?3 WHERE id = ?1",
      params![session.0, now.unix_millis(), end_text(end)],
    )?;
    Ok(())
  }
}

/// A row of `projects`: id, name, repo root and last use.
type ProjectRow = (String, String, String, i64);

fn project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectRow> {
  Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}

fn registered((id, name, repo_root, last_opened_at): ProjectRow) -> Result<RegisteredProject, StoreError> {
  Ok(RegisteredProject {
    id: parse("project id", &id)?,
    name: Name::new(&name).map_err(|error| corrupt("project name", error))?,
    repo_root: PathBuf::from(repo_root),
    last_opened_at: Timestamp::from_unix_millis(last_opened_at),
  })
}

const fn end_text(end: SessionEnd) -> &'static str {
  match end {
    SessionEnd::Clean => "clean",
    SessionEnd::RunsPaused => "runs_paused",
    SessionEnd::QuitNow => "quit_now",
  }
}

fn parse_end(value: &str) -> Result<PreviousSession, StoreError> {
  match value {
    "clean" => Ok(PreviousSession::Ended(SessionEnd::Clean)),
    "runs_paused" => Ok(PreviousSession::Ended(SessionEnd::RunsPaused)),
    "quit_now" => Ok(PreviousSession::Ended(SessionEnd::QuitNow)),
    "crashed" => Ok(PreviousSession::Crashed),
    other => Err(corrupt("session end", other)),
  }
}
