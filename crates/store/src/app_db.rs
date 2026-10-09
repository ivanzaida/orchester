//! The application database next to the executable: known projects, the session log used to
//! detect that the previous run of Orchester ended abnormally, the state of the window
//! (`ui_state`) that outlives a session, such as its tabs, the shared servers ([`servers`]) and the
//! application's own settings ([`settings`]), among them the Usage dashboard's warning threshold.

mod servers;
mod settings;

use std::path::{Path, PathBuf};

use orchester_core::{id::ProjectId, name::Name, project::Project, time::Timestamp};
use rusqlite::{Connection, OptionalExtension, params};

pub use self::servers::{ServerAccount, StoredServer};
use crate::{
  StoreError, migrate,
  rows::{corrupt, parse, path_text},
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

  /// Adds the project or, if it is already known, updates its name, location and last use.
  pub fn register_project(&mut self, project: &Project, repo_root: &Path, now: Timestamp) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO projects (id, name, repo_root, added_at, last_opened_at) VALUES (?1, ?2, ?3, ?4, ?4) \
       ON CONFLICT (id) DO UPDATE SET name = excluded.name, repo_root = excluded.repo_root, \
       last_opened_at = excluded.last_opened_at",
      params![
        project.id().to_string(),
        project.name().as_str(),
        path_text("repository path", repo_root)?,
        now.unix_millis(),
      ],
    )?;
    Ok(())
  }

  /// Forgets the project: it is no longer listed. Only this database changes; the project's
  /// folder and its `.orchester` data stay as they are, so opening the folder again brings the
  /// project back. Forgetting a project that is not known changes nothing.
  pub fn forget_project(&mut self, project: ProjectId) -> Result<(), StoreError> {
    self
      .connection
      .execute("DELETE FROM projects WHERE id = ?1", [project.to_string()])?;
    Ok(())
  }

  /// Known projects, most recently opened first.
  pub fn projects(&self) -> Result<Vec<RegisteredProject>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT id, name, repo_root, last_opened_at FROM projects ORDER BY last_opened_at DESC")?;
    let rows = statement
      .query_map([], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, i64>(3)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(id, name, repo_root, last_opened_at)| {
        Ok(RegisteredProject {
          id: parse("project id", &id)?,
          name: Name::new(&name).map_err(|error| corrupt("project name", error))?,
          repo_root: PathBuf::from(repo_root),
          last_opened_at: Timestamp::from_unix_millis(last_opened_at),
        })
      })
      .collect()
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
