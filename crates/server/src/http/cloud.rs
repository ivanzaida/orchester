//! The server's link to Orchester Cloud for its admins (`/server/cloud`).

use axum::{Json, extract::State};
use orchester_api::{
  CloudLinkChanges, CloudLinkFinish, CloudLinkResponse, CloudLinkStep, ErrorCode, KeyProofRequest, KeyProofResponse,
  cloud::TokenPurpose,
  statements::{KEY_PROOF_TYPE, KeyProof},
};
use rusqlite::Connection;

use super::extract::{Auth, Base, SafeJson};
use crate::{
  Server,
  cloud_link::{self, Link},
  db::{self, Authenticated},
  error::ServerError,
  server_keys::ServerKeys,
  settings,
};

/// `POST /server/key-proof`: the server's public key and its signature of the app's nonce and of the certificate its
/// connections present, with the id of that key, which is the server's: what an app checks before it asks Orchester
/// Cloud for a code's token for this id (`docs/architecture/server.md`, "Codes"). Nothing is stored.
pub(super) async fn key_proof(
  State(server): State<Server>,
  SafeJson(input): SafeJson<KeyProofRequest>,
) -> Result<Json<KeyProofResponse>, ServerError> {
  let nonce = input.nonce;
  let shaped = (16..=64).contains(&nonce.len())
    && nonce
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
  if !shaped {
    return Err(ErrorCode::Invalid.into());
  }
  let keys = &server.inner.keys;
  let proxied = server
    .inner
    .config
    .proxy_certificate
    .as_deref()
    .and_then(|fingerprint| orchester_api::certificate::CertificatePin::parse(&fingerprint.replace(':', "")));
  let presented = server.inner.tls.as_ref().map(|tls| tls.pin).or(proxied);
  let proof = KeyProof {
    server: keys.id,
    nonce,
    certificate: presented.map(|pin| pin.to_hex()),
    issued_at: server.now().div_euclid(1000),
  };
  Ok(Json(KeyProofResponse {
    public_key: keys.public_key(),
    proof: keys.sign(KEY_PROOF_TYPE, &proof)?,
  }))
}

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
  Base(base): Base,
  SafeJson(input): SafeJson<CloudLinkChanges>,
) -> Result<Json<CloudLinkResponse>, ServerError> {
  let state = server.inner.clone();
  let fingerprint = state.tls.as_ref().map(|tls| tls.pin.to_hex());
  let response = server
    .account_write(move |db, now| {
      let current = db::checked(db, &auth, now, true)?;
      let name = settings::name(db, &state.config.name)?;
      let statement = match input {
        CloudLinkChanges {
          link: Some(CloudLinkStep::Start(_)),
          accepting: None,
          unlink: false,
          pin: None,
        } => Some(cloud_link::statement(&state.keys, &name, &base, fingerprint, now)?),
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
  let mut link = cloud_link::link(db, &finish.root_key, &finish.key_set, current.user.id, now)?;
  let Some(token) = &finish.identity_token else {
    return Ok(());
  };
  let claims = link.verify(db, keys, token.expose(), None, now)?;
  claims.require(Some(TokenPurpose::Link))?;
  if current.user.cloud || db::user_by_subject(db, &claims.sub)?.is_some() {
    return Err(ErrorCode::Conflict.into());
  }
  db::link_subject(db, current.user.id, &claims.sub, now)?;
  cloud_link::consume(db, &claims)
}
