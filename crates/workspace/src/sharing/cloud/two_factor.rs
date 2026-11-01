//! Two-factor authentication as this computer sees it (`docs/cloud-api.md`, "Two-factor authentication"): whether the
//! account has it on and whether this computer signed in with a code (`GET /me`), which accepting an invite needs;
//! signing out to sign in again with a code when it did not (the design's *Sign in again*, `rPG8h`, whose sign-in gives
//! its own reason, `Nq3Mf`); and how few recovery codes are left after a sign-in with one.

use orchester_api::cloud::TwoFactorStatus;
use orchester_core::time::Timestamp;
use orchester_store::CloudSignInEnd;

use super::{Cloud, CloudError};

/// At most this many recovery codes left after a sign-in with one, the app says to make new ones.
pub const FEW_RECOVERY_CODES: u32 = 3;

impl Cloud {
  /// The account's two-factor authentication as the cloud reports it to this computer (`GET /me`).
  pub async fn two_factor(&self) -> Result<TwoFactorStatus, CloudError> {
    self
      .with_token(|client, token| async move { Ok(client.me(&token).await?.two_factor) })
      .await
  }

  /// Signs this computer out, as *Sign out* does, so the person signs in again with a code: the sign-in says why,
  /// naming `invite`, what the invite that needs it is to. The invite stays open at the cloud.
  pub async fn sign_out_for_code(&self, invite: Option<String>) -> Result<(), CloudError> {
    self.end_session().await;
    let forgotten = self
      .forget((CloudSignInEnd::TwoFactor, invite), Timestamp::now(), None)
      .await;
    self.changed();
    forgotten.map(drop)
  }

  /// The recovery codes left, when a recovery code finished this session's sign-in and at most
  /// [`FEW_RECOVERY_CODES`] are left. Never blocks.
  pub fn few_recovery_codes(&self) -> Option<u32> {
    self
      .inner
      .recovery_codes_left
      .lock()
      .filter(|left| *left <= FEW_RECOVERY_CODES)
  }

  /// Keeps how many recovery codes are left after this session's sign-in: `None` unless a recovery code finished it.
  pub(super) fn recovery_codes_used(&self, left: Option<u32>) {
    *self.inner.recovery_codes_left.lock() = left;
  }
}
