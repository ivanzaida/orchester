mod backup;
mod rows;
mod sessions;
mod users;

use std::{
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  time::Duration,
};

pub(crate) use backup::backup;
use parking_lot::Mutex;
pub(crate) use rows::user_row;
use rusqlite::{Connection, TransactionBehavior};
pub(crate) use sessions::{Authenticated, authenticate, checked, revoke_session, revoke_sessions, touch_session};
pub(crate) use users::{create_user, find_user, patch_user, reset_password, validate_user};

use crate::error::ServerError;

pub(crate) struct Database {
  writer: Mutex<Connection>,
  readers: Vec<Mutex<Connection>>,
  next_reader: AtomicUsize,
}

impl Database {
  pub fn open(directory: &Path) -> Result<Self, ServerError> {
    std::fs::create_dir_all(directory)?;
    let path = directory.join("orchester-server.db");
    let mut writer = connection(&path)?;
    writer.pragma_update(None, "journal_mode", "WAL")?;
    migrate(&mut writer)?;
    let readers = (0..3)
      .map(|_| {
        let reader = connection(&path)?;
        reader.pragma_update(None, "query_only", true)?;
        Ok(Mutex::new(reader))
      })
      .collect::<Result<_, ServerError>>()?;
    Ok(Self {
      writer: Mutex::new(writer),
      readers,
      next_reader: AtomicUsize::new(0),
    })
  }

  pub async fn read<T: Send + 'static>(
    self: &Arc<Self>,
    operation: impl FnOnce(&Connection) -> Result<T, ServerError> + Send + 'static,
  ) -> Result<T, ServerError> {
    let db = Arc::clone(self);
    tokio::task::spawn_blocking(move || {
      let index = db.next_reader.fetch_add(1, Ordering::Relaxed) % db.readers.len();
      operation(&db.readers[index].lock())
    })
    .await
    .map_err(|_| ServerError::Worker)?
  }

  pub async fn write<T: Send + 'static>(
    self: &Arc<Self>,
    operation: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T, ServerError> + Send + 'static,
  ) -> Result<T, ServerError> {
    let db = Arc::clone(self);
    tokio::task::spawn_blocking(move || {
      let mut connection = db.writer.lock();
      let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
      let result = operation(&transaction)?;
      transaction.commit()?;
      Ok(result)
    })
    .await
    .map_err(|_| ServerError::Worker)?
  }
}

fn connection(path: &Path) -> Result<Connection, ServerError> {
  let connection = Connection::open(path)?;
  connection.busy_timeout(Duration::from_secs(5))?;
  connection.pragma_update(None, "foreign_keys", true)?;
  Ok(connection)
}

fn migrate(connection: &mut Connection) -> Result<(), ServerError> {
  // The immediate transaction also serializes migrations with a host CLI command.
  let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
  let exists: bool = transaction.query_row(
    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='meta')",
    [],
    |row| row.get(0),
  )?;
  if exists {
    let version: i64 = transaction.query_row("SELECT schema_version FROM meta", [], |row| row.get(0))?;
    if version != 1 {
      return Err(ServerError::Configuration("unsupported server database schema"));
    }
  } else {
    transaction.execute_batch(include_str!("schema.sql"))?;
  }
  transaction.commit()?;
  Ok(())
}

pub(crate) fn audit(
  connection: &Connection,
  actor: Option<orchester_core::id::UserId>,
  action: &str,
  target: &str,
  address: Option<std::net::IpAddr>,
  now: i64,
) -> Result<(), ServerError> {
  connection.execute(
    "INSERT INTO audit_events(actor,action,target,address,at) VALUES (?1,?2,?3,?4,?5)",
    rusqlite::params![
      actor.map(|id| id.to_string()),
      action,
      target,
      address.map(|ip| ip.to_string()),
      now
    ],
  )?;
  Ok(())
}
