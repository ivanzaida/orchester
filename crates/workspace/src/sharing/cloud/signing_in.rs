//! Signing this computer in to Orchester Cloud (`docs/cloud-api.md`, "Signing in", "Two-factor authentication"): the
//! person's email and password, and for an account with two-factor authentication on, a code from the authenticator
//! app or a recovery code, which finishes the sign-in the password started ([`Challenge`]). The challenge stays in
//! memory, with whoever holds it: it is never stored, logged or shown, and a new password step replaces it.

use std::{
  fmt,
  sync::atomic::Ordering,
  time::{Duration, Instant},
};

use orchester_api::{
  Credential,
  client::{CloudAddress, CloudClient, SignInAnswer},
  cloud::{CloudSignIn, CloudSignedIn, TwoFactorSignIn},
};
use orchester_core::{secret::SecretValue, time::Timestamp};
use orchester_store::{CloudSignInEnd, StoredCloudAccount};

use super::{Cloud, CloudError, SignIn, device_key, state::Reach};
use crate::{background, sharing::device};

/// How a sign-in went on after the password: signed in, or waiting for a code.
#[derive(Debug)]
pub enum SignInStep {
  SignedIn(StoredCloudAccount),
  /// The password was right and the account has two-factor authentication on: [`Cloud::finish_sign_in`] with a code.
  Code(Challenge),
}

/// How long the cloud takes codes for a challenge, from when it gave it (`docs/cloud-api.md`, "Signing in").
pub const CHALLENGE_LIFE: Duration = Duration::from_secs(5 * 60);

/// A sign-in the password started, waiting for a two-factor code: what the cloud gave for it, valid once and for
/// [`CHALLENGE_LIFE`], on the cloud that gave it. Its `Debug` leaves the challenge out.
pub struct Challenge {
  challenge: Credential,
  /// When it arrived, on this computer's monotonic clock: its wall clock may be off from the cloud's, so the cloud's
  /// `expires_at` is not compared with it.
  received: Instant,
  email: String,
  device_name: String,
  address: CloudAddress,
}

impl Challenge {
  /// The account's email, as the person typed it.
  pub fn email(&self) -> &str {
    &self.email
  }

  /// Whether its life has passed by `now`, counted from when it arrived; the cloud's `challenge_expired` says so as
  /// well.
  pub fn ended_by(&self, now: Instant) -> bool {
    now.saturating_duration_since(self.received) >= CHALLENGE_LIFE
  }
}

impl fmt::Debug for Challenge {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Challenge")
      .field("email", &self.email)
      .finish_non_exhaustive()
  }
}

/// Which kind of code finishes a sign-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeKind {
  /// The six digits the authenticator app shows.
  App,
  /// One of the recovery codes saved when two-factor authentication was turned on; each works once.
  Recovery,
}

/// A sign-in a code finished, and how many recovery codes the account has left.
#[derive(Debug)]
pub struct CodeSignedIn {
  pub account: StoredCloudAccount,
  pub recovery_codes_left: u32,
}

impl Cloud {
  /// Signs this computer in to the cloud with the person's `email` and `password`, in place of any account signed in
  /// before; an account with two-factor authentication on answers a [`Challenge`] instead, and nothing changes here
  /// until a code finishes it. `invalid_credentials`, `email_unconfirmed` and `rate_limited` come back as the cloud's
  /// refusal.
  pub async fn sign_in(&self, email: String, password: Credential) -> Result<SignInStep, CloudError> {
    let cloud = background::run({
      let cloud = self.clone();
      move || cloud.address()
    })
    .await??;
    let client = CloudClient::new(cloud.address.clone())?;
    let device = device::cloud_device();
    let answer = client
      .sign_in(&CloudSignIn {
        email: email.clone(),
        password,
        device: device.clone(),
      })
      .await?;
    match answer {
      SignInAnswer::SignedIn(signed_in) => {
        let account = self
          .keep_sign_in(signed_in, (device.name, &cloud.address), None)
          .await?;
        Ok(SignInStep::SignedIn(account))
      }
      SignInAnswer::Challenged(challenge) => Ok(SignInStep::Code(Challenge {
        challenge: challenge.challenge,
        received: Instant::now(),
        email,
        device_name: device.name,
        address: cloud.address,
      })),
    }
  }

  /// Finishes the sign-in `challenge` started with `code` of `kind`, at the cloud that gave the challenge. A wrong code
  /// is `invalid_code` and the challenge may be tried again; one that ended is `challenge_expired`, and the person
  /// signs in again with the password; too many wrong codes are `rate_limited`.
  pub async fn finish_sign_in(
    &self,
    challenge: &Challenge,
    (code, kind): (Credential, CodeKind),
  ) -> Result<CodeSignedIn, CloudError> {
    let client = CloudClient::new(challenge.address.clone())?;
    let finished = client
      .finish_sign_in(&TwoFactorSignIn {
        challenge: Credential::new(challenge.challenge.expose().to_owned()),
        code,
      })
      .await?;
    let left = finished.recovery_codes_left;
    let recovery = (kind == CodeKind::Recovery).then_some(left);
    let account = self
      .keep_sign_in(
        finished.signed_in,
        (challenge.device_name.clone(), &challenge.address),
        recovery,
      )
      .await?;
    Ok(CodeSignedIn {
      account,
      recovery_codes_left: left,
    })
  }

  /// Keeps the sign-in the cloud at `address` answered, under the device name `device_name`: its token in the
  /// keychain and its account in the database. It claims nothing: this computer's projects and servers show beside the
  /// account's own. `recovery`: the recovery codes left, when one finished the sign-in.
  async fn keep_sign_in(
    &self,
    signed_in: CloudSignedIn,
    (device_name, address): (String, &CloudAddress),
    recovery: Option<u32>,
  ) -> Result<StoredCloudAccount, CloudError> {
    // The cloud issues `orcc_` and 64 hex characters; a blank answer is not a token.
    let token = SecretValue::new(signed_in.token.expose().to_owned()).map_err(|_| CloudError::SignedOut)?;
    if self.account_now().await?.is_some() {
      self.end_session().await;
    }
    let account = StoredCloudAccount {
      account: signed_in.account.id,
      email: signed_in.account.email,
      display_name: signed_in.account.display_name,
      device_name,
      signed_in_at: Timestamp::now(),
      cloud_address: address.web().as_str().to_owned(),
      cloud_api_address: address.api().map(|api| api.as_str().to_owned()),
    };
    let (cloud, kept) = (self.clone(), account.clone());
    background::run(move || -> Result<(), CloudError> {
      let inner = &cloud.inner;
      let _keychain = inner.keychain.lock();
      let key = device_key(&inner.workspace)?;
      inner.vault.get().store(&key, &token)?;
      let mut db = inner.workspace.db.lock();
      inner.generation.fetch_add(1, Ordering::SeqCst);
      // Invites kept through a sign-out for a code are that account's alone.
      let ended = db.cloud_sign_in_ended()?;
      if ended.is_some_and(|ended| ended.reason == CloudSignInEnd::TwoFactor && ended.email != kept.email) {
        db.forget_accepted_invites()?;
      }
      db.set_cloud_account(Some(&kept))?;
      db.set_cloud_sign_in_ended(None)?;
      Ok(())
    })
    .await??;
    self.recovery_codes_used(recovery);
    self.inner.reach.send_replace(Reach::Reached);
    self.inner.sign_in.send_replace(SignIn::SignedIn {
      account: account.account,
      email: account.email.clone(),
    });
    self.changed();
    // The server lists show this account's servers from now on.
    self.servers().changed();
    Ok(account)
  }
}
