//! The sign-in gate's side of the cloud account (the person's decision, 2026-10-16): the app is used only while this
//! computer is signed in to Orchester Cloud. A sign-in that was made keeps working without the internet; it is checked
//! again whenever the cloud can be reached ([`keep_checking`]), and a refusal ends it with the cloud's reason
//! (`docs/cloud-api.md`, "Why a token ended"). [`SignIn`] and [`Reach`] are published for the window and the app's
//! MCP server.

use std::time::Duration;

use orchester_api::cloud::SessionEnd;
use orchester_core::time::Timestamp;
use orchester_store::{CloudSignInEnd, EndedCloudSignIn};

use super::{Cloud, CloudError};

/// How often a signed-in computer asks the cloud whether its sign-in still holds, while the cloud answers.
pub const CHECK_EVERY: Duration = Duration::from_secs(5 * 60);
/// How often it tries again while the cloud cannot be reached.
pub const RETRY_EVERY: Duration = Duration::from_secs(30);

/// Whether this computer is signed in to Orchester Cloud.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignIn {
  /// The app is open.
  SignedIn { email: String },
  /// Only the sign-in gate shows, since `since`; `ended` says why the last sign-in from here ended, if one did.
  SignedOut {
    ended: Option<EndedCloudSignIn>,
    since: Timestamp,
  },
}

impl SignIn {
  pub const fn is_signed_in(&self) -> bool {
    matches!(self, Self::SignedIn { .. })
  }
}

/// Whether Orchester Cloud answered this computer's last request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
  /// Nothing was asked since the sign-in or the start.
  Unknown,
  Reached,
  /// No answer since `since`.
  Unreachable {
    since: Timestamp,
  },
}

impl Reach {
  /// After `answer`'s request: reached when the cloud answered at all, unreachable since the first of a run of
  /// requests without an answer.
  pub(super) fn after<T>(self, answer: &Result<T, CloudError>, now: Timestamp) -> Self {
    match answer {
      Ok(_) => Self::Reached,
      Err(error) if error.cloud_code().is_some() => Self::Reached,
      Err(error) if error.unanswered() => match self {
        Self::Unreachable { since } => Self::Unreachable { since },
        Self::Unknown | Self::Reached => Self::Unreachable { since: now },
      },
      // Nothing went out, or the work here failed: it says nothing about reaching the cloud.
      Err(_) => self,
    }
  }
}

/// The reason a refusal of this computer's token carries, as the gate shows it.
pub(super) const fn end_of(reason: Option<SessionEnd>) -> CloudSignInEnd {
  match reason {
    Some(SessionEnd::Revoked) => CloudSignInEnd::Revoked,
    Some(SessionEnd::PasswordReset) => CloudSignInEnd::PasswordReset,
    Some(SessionEnd::AccountDeleted) => CloudSignInEnd::AccountDeleted,
    Some(SessionEnd::SignedOut) => CloudSignInEnd::SignedOut,
    Some(SessionEnd::Other) | None => CloudSignInEnd::Other,
  }
}

impl Cloud {
  /// Whether this computer is signed in now. Never blocks.
  pub fn sign_in_state(&self) -> SignIn {
    self.inner.sign_in.borrow().clone()
  }

  pub fn is_signed_in(&self) -> bool {
    self.inner.sign_in.borrow().is_signed_in()
  }

  /// Wakes whenever this computer signs in or its sign-in ends.
  pub fn follow_sign_in(&self) -> tokio::sync::watch::Receiver<SignIn> {
    self.inner.sign_in.subscribe()
  }

  /// Whether the cloud answered lately. Never blocks.
  pub fn reach(&self) -> Reach {
    *self.inner.reach.borrow()
  }

  /// Wakes whenever the cloud starts or stops answering.
  pub fn follow_reach(&self) -> tokio::sync::watch::Receiver<Reach> {
    self.inner.reach.subscribe()
  }

  /// Asks the cloud whether this computer's sign-in still holds (`GET /me`). A refusal ends it with the cloud's
  /// reason ([`CloudError::SignedOut`]); without an answer it stays as it was, and [`Self::reach`] says so.
  pub async fn check_sign_in(&self) -> Result<(), CloudError> {
    self
      .with_token(|client, token| async move { client.me(&token).await.map(drop).map_err(CloudError::from) })
      .await
  }
}

/// Checks this computer's sign-in at once and then while it lasts: every [`CHECK_EVERY`] while the cloud answers,
/// every [`RETRY_EVERY`] while it cannot be reached, and again whenever the account signs in. Runs until the session
/// ends.
pub async fn keep_checking(cloud: Cloud) {
  let mut sign_in = cloud.follow_sign_in();
  loop {
    let signed_in = sign_in.borrow_and_update().is_signed_in();
    if !signed_in {
      if sign_in.changed().await.is_err() {
        return;
      }
      continue;
    }
    let wait = match cloud.check_sign_in().await {
      Ok(()) => CHECK_EVERY,
      Err(error) if error.unanswered() => RETRY_EVERY,
      Err(CloudError::SignedOut) => continue,
      Err(error) => {
        tracing::warn!(%error, "cannot check this computer's sign-in to Orchester Cloud");
        CHECK_EVERY
      }
    };
    tokio::select! {
      () = tokio::time::sleep(wait) => {}
      changed = sign_in.changed() => if changed.is_err() { return },
    }
  }
}
