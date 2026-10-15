//! The server's settings for its admins: the cloud link (`/server/cloud`) and join approval (`/server/settings`).

use axum::{Json, extract::State, http::HeaderMap};
use orchester_api::{
  CloudLinkChanges, CloudLinkFinish, CloudLinkResponse, CloudLinkStep, ErrorCode, ServerSettings, SettingsChanges,
  SettingsResponse,
};
use rusqlite::Connection;

use super::{
  base_address,
  extract::{Auth, SafeJson},
};
use crate::{
  Server, approvals,
  cloud_link::{self, Link},
  db::{self, Authenticated},
  error::ServerError,
  server_keys::ServerKeys,
};

pub(super) async fn show_cloud(
  State(server): State<Server>,
  Auth(auth): Auth,
) -> Result<Json<CloudLinkResponse>, ServerError> {
  let state = server.inner.clone();
  let cloud = server
    .snapshot(move |db, now| {
      db::checked(db, &auth, now, true)?;
      Ok(Link::describe(Link::load(db)?.as_ref(), &state.keys))
    })
    .await?;
  Ok(Json(CloudLinkResponse { cloud, statement: None }))
}

/// `PATCH /server/cloud`: one change at a time, audited.
pub(super) async fn change_cloud(
  State(server): State<Server>,
  Auth(auth): Auth,
  headers: HeaderMap,
  SafeJson(input): SafeJson<CloudLinkChanges>,
) -> Result<Json<CloudLinkResponse>, ServerError> {
  let base = base_address(&server, &headers);
  let state = server.inner.clone();
  let check_code = state.tls.as_ref().map(|tls| tls.pin.check_code());
  let response = server
    .account_write(move |db, now| {
      let current = db::checked(db, &auth, now, true)?;
      let statement = match input {
        CloudLinkChanges {
          link: Some(CloudLinkStep::Start(_)),
          accepting: None,
          unlink: false,
          pin: None,
        } => Some(cloud_link::statement(
          &state.keys,
          &state.config.name,
          &base,
          check_code,
          now,
        )?),
        other => {
          change(db, &state.keys, &current, other, now)?;
          None
        }
      };
      let cloud = Link::describe(Link::load(db)?.as_ref(), &state.keys);
      Ok(CloudLinkResponse { cloud, statement })
    })
    .await?;
  Ok(Json(response))
}

fn change(
  db: &Connection,
  keys: &ServerKeys,
  current: &Authenticated,
  input: CloudLinkChanges,
  now: i64,
) -> Result<(), ServerError> {
  let action = match input {
    CloudLinkChanges {
      link: Some(CloudLinkStep::Finish(finish)),
      accepting: None,
      unlink: false,
      pin: None,
    } => {
      finish_link(db, keys, current, &finish, now)?;
      "cloud_linked"
    }
    CloudLinkChanges {
      accepting: Some(accepting),
      link: None,
      unlink: false,
      pin: None,
    } => {
      cloud_link::set_accepting(db, accepting)?;
      if accepting { "cloud_on" } else { "cloud_off" }
    }
    CloudLinkChanges {
      pin: Some(root),
      link: None,
      accepting: None,
      unlink: false,
    } => {
      cloud_link::pin(db, &root, now)?;
      "cloud_pinned"
    }
    CloudLinkChanges {
      unlink: true,
      link: None,
      accepting: None,
      pin: None,
    } => {
      cloud_link::unlink(db)?;
      "cloud_unlinked"
    }
    _ => return Err(ErrorCode::Invalid.into()),
  };
  db::audit(db, Some(current.user.id), action, "cloud", None, now)
}

/// Pins the cloud's root with its key set; with an identity token, links the calling admin's account too.
fn finish_link(
  db: &Connection,
  keys: &ServerKeys,
  current: &Authenticated,
  finish: &CloudLinkFinish,
  now: i64,
) -> Result<(), ServerError> {
  let mut link = cloud_link::link(db, &finish.root_key, &finish.key_set, now)?;
  let Some(token) = &finish.identity_token else {
    return Ok(());
  };
  let claims = link.verify(db, keys, token.expose(), None, now)?;
  if current.user.cloud || db::user_by_subject(db, &claims.sub)?.is_some() {
    return Err(ErrorCode::Conflict.into());
  }
  db::link_subject(db, current.user.id, &claims.sub, now)?;
  cloud_link::consume(db, &claims)
}

pub(super) async fn settings(
  State(server): State<Server>,
  Auth(auth): Auth,
) -> Result<Json<SettingsResponse>, ServerError> {
  let check_code = server.check_code();
  let join_approval = server
    .snapshot(move |db, now| {
      db::checked(db, &auth, now, true)?;
      approvals::enabled(db)
    })
    .await?;
  Ok(Json(SettingsResponse {
    settings: ServerSettings { join_approval },
    check_code,
  }))
}

pub(super) async fn change_settings(
  State(server): State<Server>,
  Auth(auth): Auth,
  SafeJson(input): SafeJson<SettingsChanges>,
) -> Result<Json<SettingsResponse>, ServerError> {
  let check_code = server.check_code();
  let join_approval = server
    .account_write(move |db, now| {
      let current = db::checked(db, &auth, now, true)?;
      if let Some(enabled) = input.join_approval {
        approvals::set_enabled(db, enabled, current.user.id, now)?;
      }
      approvals::enabled(db)
    })
    .await?;
  Ok(Json(SettingsResponse {
    settings: ServerSettings { join_approval },
    check_code,
  }))
}
