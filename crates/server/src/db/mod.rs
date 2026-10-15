mod backup;
mod migrate;
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
pub use backup::backup_existing;
use parking_lot::Mutex;
pub(crate) use rows::{USER_COLUMNS, user_row};
use rusqlite::{Connection, TransactionBehavior};
pub(crate) use sessions::{
  Authenticated, NewSession, authenticate, checked, insert_session, parse_via, revoke_session, revoke_sessions,
  touch_session,
};
pub(crate) use users::{
  NewUser, create_user, find_user, has_admin, link_subject, patch_user, user_by_id, user_by_subject,
};

use crate::error::ServerError;

pub(crate) struct Database {
  writer: Mutex<Connection>,
  readers: Vec<Mutex<Connection>>,
  next_reader: AtomicUsize,
  changes: tokio::sync::watch::Sender<u64>,
}

impl Database {
  /// Opens the database in `directory`, created or migrated at `now`.
  pub fn open(directory: &Path, now: i64) -> Result<Self, ServerError> {
    std::fs::create_dir_all(directory)?;
    let path = directory.join("orchester-server.db");
    let mut writer = connection(&path)?;
    writer.pragma_update(None, "journal_mode", "WAL")?;
    migrate::migrate(&mut writer, now)?;
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
      changes: tokio::sync::watch::channel(0).0,
    })
  }

  /// Runs `operation` on a reader on the calling thread, while the server is being opened.
  pub fn read_now<T>(&self, operation: impl FnOnce(&Connection) -> Result<T, ServerError>) -> Result<T, ServerError> {
    operation(&self.readers[0].lock())
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
      db.changes
        .send_modify(|generation| *generation = generation.wrapping_add(1));
      Ok(result)
    })
    .await
    .map_err(|_| ServerError::Worker)?
  }

  pub async fn snapshot<T: Send + 'static>(
    self: &Arc<Self>,
    operation: impl FnOnce(&Connection) -> Result<T, ServerError> + Send + 'static,
  ) -> Result<T, ServerError> {
    self
      .read(move |db| {
        let transaction = db.unchecked_transaction()?;
        let result = operation(&transaction)?;
        transaction.commit()?;
        Ok(result)
      })
      .await
  }

  pub fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
    self.changes.subscribe()
  }
}

fn connection(path: &Path) -> Result<Connection, ServerError> {
  let connection = Connection::open(path)?;
  connection.busy_timeout(Duration::from_secs(5))?;
  connection.pragma_update(None, "foreign_keys", true)?;
  Ok(connection)
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
