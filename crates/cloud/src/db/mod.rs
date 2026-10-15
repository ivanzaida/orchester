//! The cloud's PostgreSQL (`docs/architecture/server.md`, row 54): a small pool of `tokio-postgres` connections, TLS
//! through rustls, plain SQL with typed parameters. Rules that need atomicity are single statements that compare and
//! set (`… WHERE used_at IS NULL RETURNING`, `INSERT … ON CONFLICT`), so no transaction needs a retry.

mod migrate;
mod tls;

use std::{
  net::IpAddr,
  str::FromStr,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use deadpool_postgres::{GenericClient, Manager, ManagerConfig, Object, Pool, RecyclingMethod, Runtime};
pub use migrate::LATEST_SCHEMA;
use orchester_api::Credential;
use orchester_core::id::CloudAccountId;

use crate::error::CloudError;

pub(crate) struct Database {
  pool: Pool,
}

impl Database {
  /// Connects and applies the migrations this binary carries; a database at a newer version is refused.
  pub async fn open(url: &Credential, now: i64) -> Result<Self, CloudError> {
    // The parser's message may quote the connection string, which holds the password.
    let config = tokio_postgres::Config::from_str(url.expose())
      .map_err(|_| CloudError::Configuration("DATABASE_URL is not a PostgreSQL connection string"))?;
    let manager = Manager::from_config(
      config,
      tls::connector()?,
      ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
      },
    );
    let pool = Pool::builder(manager)
      .max_size(16)
      .runtime(Runtime::Tokio1)
      .create_timeout(Some(Duration::from_secs(10)))
      .wait_timeout(Some(Duration::from_secs(10)))
      .recycle_timeout(Some(Duration::from_secs(5)))
      .build()
      .map_err(|_| CloudError::Configuration("cannot build the database pool"))?;
    let database = Self { pool };
    let mut client = database.client().await?;
    migrate::migrate(&mut client, time(now)).await?;
    Ok(database)
  }

  pub async fn client(&self) -> Result<Object, CloudError> {
    Ok(self.pool.get().await?)
  }
}

pub(crate) fn time(ms: i64) -> SystemTime {
  UNIX_EPOCH + Duration::from_millis(ms.max(0).unsigned_abs())
}

pub(crate) fn millis(time: SystemTime) -> Result<i64, CloudError> {
  let elapsed = time.duration_since(UNIX_EPOCH).map_err(|_| CloudError::StoredData)?;
  i64::try_from(elapsed.as_millis()).map_err(|_| CloudError::StoredData)
}

pub(crate) async fn audit(
  client: &impl GenericClient,
  actor: Option<CloudAccountId>,
  action: &str,
  target: &str,
  address: Option<IpAddr>,
  now: i64,
) -> Result<(), CloudError> {
  client
    .execute(
      "INSERT INTO audit_events (actor, action, target, address, at) VALUES ($1, $2, $3, $4, $5)",
      &[
        &actor.map(CloudAccountId::as_uuid),
        &action,
        &target,
        &address,
        &time(now),
      ],
    )
    .await?;
  Ok(())
}
