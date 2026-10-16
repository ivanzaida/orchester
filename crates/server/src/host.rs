//! What the host command line does (`docs/architecture/server.md`, "Platforms"): codes for the person at the
//! server's console, the certificate, and the owner's control of the cloud link. Codes are returned to the command,
//! which prints them only to its own output.

use orchester_api::{CloudLink, Credential, ErrorCode, certificate::CertificatePin};

use crate::{
  Server,
  cloud_link::{self, Link},
  codes::{self, CodeKind, NewCode},
  config::Config,
  db,
  error::ServerError,
  tls::certificate,
};

/// `orchester-server cloud …`.
pub enum CloudCommand {
  Show,
  On,
  Off,
  /// Replaces the pinned root with this base64url public key.
  Pin(String),
  Unlink,
}

impl Server {
  /// Creates an online SQLite snapshot at a new path while the server may be running.
  pub async fn host_backup(&self, path: std::path::PathBuf) -> Result<(), ServerError> {
    self.inner.db.read(move |source| db::backup(source, &path)).await
  }

  /// A fresh setup code while the server has no admin, ending any earlier one; `None` once it has an admin.
  pub async fn host_setup_code(&self) -> Result<Option<Credential>, ServerError> {
    let state = self.inner.clone();
    self
      .account_write(move |db, now| {
        if db::has_admin(db)? {
          return Ok(None);
        }
        codes::end(db, CodeKind::Setup, None)?;
        let new = NewCode {
          kind: CodeKind::Setup,
          invite: None,
          user: None,
          made_by: None,
          mfa: false,
        };
        let (code, _) = codes::create(db, &state.keys, &new, now)?;
        db::audit(db, None, "code_made", "setup", None, now)?;
        Ok(Some(code))
      })
      .await
  }

  /// An account code for `username`, whose person lost every device (`orchester-server account-code`).
  pub async fn host_account_code(&self, username: String) -> Result<Credential, ServerError> {
    let state = self.inner.clone();
    self
      .account_write(move |db, now| {
        let user = db::find_user(db, &username)?
          .filter(|user| !user.disabled)
          .ok_or(ErrorCode::NoSuchUser)?;
        let new = NewCode {
          kind: CodeKind::Account,
          invite: None,
          user: Some(user.id),
          made_by: None,
          mfa: false,
        };
        let (code, _) = codes::create(db, &state.keys, &new, now)?;
        db::audit(db, None, "code_made", &format!("account:{}", user.id), None, now)?;
        Ok(code)
      })
      .await
  }

  /// `orchester-server cloud show|on|off|pin|unlink`: takes effect at once, also on a running server.
  pub async fn host_cloud(&self, command: CloudCommand) -> Result<CloudLink, ServerError> {
    let state = self.inner.clone();
    self
      .account_write(move |db, now| {
        let action = match command {
          CloudCommand::Show => None,
          CloudCommand::On => Some(cloud_link::set_accepting(db, true).map(|()| "cloud_on")),
          CloudCommand::Off => Some(cloud_link::set_accepting(db, false).map(|()| "cloud_off")),
          CloudCommand::Pin(root) => Some(cloud_link::pin(db, &root, now).map(|()| "cloud_pinned")),
          CloudCommand::Unlink => Some(cloud_link::unlink(db).map(|()| "cloud_unlinked")),
        };
        if let Some(action) = action.transpose()? {
          db::audit(db, None, action, "host", None, now)?;
        }
        Ok(Link::describe(Link::load(db)?.as_ref(), &state.keys))
      })
      .await
  }
}

/// The pin of the certificate the server presents, generating its own first if it has none yet; `None` for plain
/// HTTP (`orchester-server tls show` prints its check code and SHA-256 fingerprint).
pub fn tls_pin(config: &Config, now_ms: i64) -> Result<Option<CertificatePin>, ServerError> {
  Ok(certificate::load(config, now_ms)?.map(|material| material.pin))
}

/// Replaces the server's own certificate and returns the new check code (`orchester-server tls regenerate`). A running
/// server presents the new one after its next start.
pub fn tls_regenerate(config: &Config, now_ms: i64) -> Result<String, ServerError> {
  Ok(certificate::regenerate(config, now_ms)?.pin.check_code())
}
