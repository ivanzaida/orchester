//! Forward-only SQL migrations compiled into the binary, each applied in its own transaction under one advisory lock,
//! so instances that start together apply each once (`docs/architecture/server.md`, "The cloud's migrations").

use std::time::SystemTime;

use orchester_api::identity::PublicKey;
use tokio_postgres::{Client, Transaction};
use uuid::Uuid;

use crate::error::CloudError;

const MIGRATIONS: &[(i32, &str)] = &[
  (1, include_str!("../../migrations/0001_accounts.sql")),
  (2, include_str!("../../migrations/0002_pending_sign_up.sql")),
  (3, include_str!("../../migrations/0003_servers_invites.sql")),
  (4, include_str!("../../migrations/0004_registry_ownership.sql")),
];

/// The newest schema version this binary knows.
pub const LATEST_SCHEMA: i32 = MIGRATIONS[MIGRATIONS.len() - 1].0;

/// "orchclou" in ASCII: the advisory lock only the cloud's migrations take.
const LOCK: i64 = 0x6f72_6368_636c_6f75;

pub(super) async fn migrate(client: &mut Client, now: SystemTime) -> Result<(), CloudError> {
  for (version, sql) in MIGRATIONS {
    let transaction = client.transaction().await?;
    transaction
      .execute("SELECT pg_advisory_xact_lock($1)", &[&LOCK])
      .await?;
    transaction
      .batch_execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (version integer PRIMARY KEY, applied_at timestamptz NOT NULL)",
      )
      .await?;
    let current: i32 = transaction
      .query_one("SELECT coalesce(max(version), 0) FROM schema_migrations", &[])
      .await?
      .get(0);
    if current > LATEST_SCHEMA {
      return Err(CloudError::SchemaTooNew(current));
    }
    if current < *version {
      transaction.batch_execute(sql).await?;
      if *version == 4 {
        drop_foreign_ids(&transaction).await?;
      }
      transaction
        .execute(
          "INSERT INTO schema_migrations (version, applied_at) VALUES ($1, $2)",
          &[version, &now],
        )
        .await?;
      tracing::info!(version, "database migration applied");
    }
    transaction.commit().await?;
  }
  Ok(())
}

/// A server's id is its key's (security review 1, B1). A server linked under migration 3 with another id holds an id
/// it may not own, and it would keep the key that id belongs to from linking: its row goes, with its accounts and
/// invites, and the log names it. The server links again under its own id (security review 2, 3).
async fn drop_foreign_ids(transaction: &Transaction<'_>) -> Result<(), CloudError> {
  let rows = transaction.query("SELECT id, public_key FROM servers", &[]).await?;
  for row in rows {
    let id: Uuid = row.try_get(0)?;
    let key: String = row.try_get(1)?;
    let own = PublicKey::parse(&key).is_ok_and(|key| key.server_id().as_uuid() == id);
    if !own {
      transaction.execute("DELETE FROM servers WHERE id = $1", &[&id]).await?;
      tracing::warn!(server = %id, "a linked server's id is not its key's; its link is removed");
    }
  }
  Ok(())
}
