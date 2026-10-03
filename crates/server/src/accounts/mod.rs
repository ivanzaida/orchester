mod admin;
mod auth;
mod password;

use std::sync::Arc;

use orchester_api::{CreateUser, Credential, ErrorCode, UserCreated};

use crate::{
  clock::Clock,
  config::Config,
  db::{self, Database},
  error::ServerError,
  passwords::{Passwords, random_credential},
  rate_limit::RateLimit,
};

#[derive(Clone)]
pub struct Server {
  pub(crate) inner: Arc<State>,
}

pub(crate) struct State {
  pub config: Config,
  pub clock: Arc<dyn Clock>,
  pub db: Arc<Database>,
  pub passwords: Passwords,
  pub limits: RateLimit,
}

impl Server {
  pub async fn open(config: Config, clock: Arc<dyn Clock>) -> Result<Self, ServerError> {
    config.validate()?;
    let directory = config.data_dir.clone();
    let db = tokio::task::spawn_blocking(move || Database::open(&directory))
      .await
      .map_err(|_| ServerError::Worker)??;
    Ok(Self {
      inner: Arc::new(State {
        config,
        clock,
        db: Arc::new(db),
        passwords: Passwords::new(),
        limits: RateLimit::default(),
      }),
    })
  }

  pub(crate) fn now(&self) -> i64 {
    self.inner.clock.now_ms()
  }

  /// Creates an online SQLite snapshot at a new path while the server may be running.
  pub async fn host_backup(&self, path: std::path::PathBuf) -> Result<(), ServerError> {
    self.inner.db.read(move |source| db::backup(source, &path)).await
  }

  pub async fn host_create_user(&self, input: CreateUser) -> Result<UserCreated, ServerError> {
    db::validate_user(&input)?;
    let initial_password = random_credential("", 16)?;
    let hash = self
      .inner
      .passwords
      .hash(Credential::new(initial_password.expose().into()))
      .await?;
    let now = self.now();
    let user = self
      .inner
      .db
      .write(move |tx| {
        let admins: i64 = tx.query_row(
          "SELECT COUNT(*) FROM users WHERE admin=1 AND disabled_at IS NULL",
          [],
          |row| row.get(0),
        )?;
        if admins == 0 && !input.admin {
          return Err(ErrorCode::Invalid.into());
        }
        db::create_user(tx, &input, &hash, None, now)
      })
      .await?;
    Ok(UserCreated { user, initial_password })
  }

  pub async fn host_reset_password(&self, username: String) -> Result<Credential, ServerError> {
    let initial = random_credential("", 16)?;
    let hash = self
      .inner
      .passwords
      .hash(Credential::new(initial.expose().into()))
      .await?;
    let now = self.now();
    self
      .inner
      .db
      .write(move |tx| {
        let user = db::find_user(tx, &username)?.ok_or(ErrorCode::NotFound)?;
        db::reset_password(tx, user.user.id, &hash, None, now)
      })
      .await?;
    Ok(initial)
  }
}
