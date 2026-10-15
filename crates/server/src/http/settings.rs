//! `/server/settings` for admins: the server's name, join approval, and the certificate people compare.

use axum::{Json, extract::State};
use orchester_api::{ServerSettings, SettingsChanges, SettingsResponse};

use super::extract::{Auth, SafeJson};
use crate::{Server, approvals, db, error::ServerError, settings};

pub(super) async fn show(
  State(server): State<Server>,
  Auth(auth): Auth,
) -> Result<Json<SettingsResponse>, ServerError> {
  let configured = server.inner.config.name.clone();
  let settings = server
    .snapshot(move |db, now| {
      db::checked(db, &auth, now, true)?;
      current(db, &configured)
    })
    .await?;
  Ok(Json(response(&server, settings)))
}

pub(super) async fn change(
  State(server): State<Server>,
  Auth(auth): Auth,
  SafeJson(input): SafeJson<SettingsChanges>,
) -> Result<Json<SettingsResponse>, ServerError> {
  let configured = server.inner.config.name.clone();
  let settings = server
    .account_write(move |db, now| {
      let actor = db::checked(db, &auth, now, true)?.user.id;
      if let Some(enabled) = input.join_approval {
        approvals::set_enabled(db, enabled, actor, now)?;
      }
      if let Some(name) = &input.name {
        settings::set_name(db, name, actor, now)?;
      }
      current(db, &configured)
    })
    .await?;
  Ok(Json(response(&server, settings)))
}

fn current(db: &rusqlite::Connection, configured: &str) -> Result<ServerSettings, ServerError> {
  Ok(ServerSettings {
    join_approval: approvals::enabled(db)?,
    name: settings::name(db, configured)?,
  })
}

fn response(server: &Server, settings: ServerSettings) -> SettingsResponse {
  SettingsResponse {
    settings,
    check_code: server.check_code(),
    certificate: server.certificate(),
  }
}
