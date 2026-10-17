//! Accepting an invite through Orchester Cloud (`docs/cloud-api.md`, "Accepting an invite"): this computer reads the
//! invite's server from the cloud's registry, which holds the address and certificate fingerprint the server linked
//! with, and joins it with an identity token that names the invite, trusting only that certificate there, so there is
//! no check code to compare. The cloud lists an accepted invite no more but keeps issuing tokens for it to this
//! account, so this computer keeps the invite until the join finished (N1 security review 1, M2): a join that failed is
//! asked again, and one that waits for approval is asked again with that approval until it is approved or refused,
//! as often as [`super::asking`] lets it. Accepting needs two-factor authentication on the account and a sign-in with a
//! code on this computer: a refusal for either leaves the invite open, at the cloud and here.

use orchester_api::{
  ErrorCode,
  client::ClientError,
  cloud::{CloudErrorCode, CloudInvite, CloudServer},
};
use orchester_core::{id::InviteId, time::Timestamp};
use orchester_store::AcceptedInvite;

use super::{
  Cloud, CloudError,
  asking::{Asker, approval_open, asking, may_ask, scheduled},
  invites::ended,
};
use crate::{
  background,
  sharing::{CloudIdentity, CloudJoined, SharingError},
};

/// An invite accepted: the server as the cloud registered it, the project the invite was to, by name, and how the
/// join went: joined, or waiting for approval.
#[derive(Debug)]
pub struct Accepted {
  pub server: CloudServer,
  pub project: Option<String>,
  pub joined: CloudJoined,
}

impl Cloud {
  /// Accepts `invite` and joins its server. The invite is kept here until the join finished: when it waits for
  /// approval or fails on the way, [`Self::finish_accept`] asks again.
  pub async fn accept(&self, invite: &CloudInvite) -> Result<Accepted, CloudError> {
    let now = Timestamp::now();
    let kept = AcceptedInvite::new(
      (invite.id, invite.server.id, invite.server.name.clone()),
      invite.project.as_ref().map(|project| project.name.clone()),
      Timestamp::from_unix_millis(invite.expires_at),
      now,
    );
    self.join_accepted(kept, Asker::Person, now).await
  }

  /// The person asks again for `invite`, accepted here before: after a join that failed, or while it waits for
  /// approval.
  pub async fn finish_accept(&self, invite: InviteId) -> Result<Accepted, CloudError> {
    let kept = self
      .accepted_invites()
      .await?
      .into_iter()
      .find(|kept| kept.invite == invite)
      .ok_or(CloudError::InviteEnded)?;
    self.join_accepted(kept, Asker::Person, Timestamp::now()).await
  }

  /// The invites accepted here whose join has not finished, oldest first.
  pub async fn accepted_invites(&self) -> Result<Vec<AcceptedInvite>, CloudError> {
    let workspace = self.inner.workspace.clone();
    Ok(background::run(move || workspace.db.lock().accepted_invites(Timestamp::now())).await??)
  }

  /// Sets `invite` aside (*Cancel joining*): nothing more is asked for it from this computer.
  pub async fn set_aside(&self, invite: InviteId) -> Result<(), CloudError> {
    self.forget_accepted(invite).await?;
    self.changed();
    Ok(())
  }

  /// Asks the team server for `kept` at `now`, the ask counted and kept before its token is asked for, so one that
  /// stops halfway counts as well.
  pub(super) async fn join_accepted(
    &self,
    kept: AcceptedInvite,
    asker: Asker,
    now: Timestamp,
  ) -> Result<Accepted, CloudError> {
    if !may_ask(&kept) {
      return Err(CloudError::TooManyAsks);
    }
    // No earlier ask took a token for it, so the cloud has not marked it accepted and still lists it.
    let first = kept.asks == 0;
    let kept = asking(kept, asker);
    self.keep_accepted(kept.clone()).await?;
    let outcome = self.ask(&kept, now).await;
    match &outcome {
      Ok(accepted) => match &accepted.joined {
        CloudJoined::Joined(_) => self.forget_accepted(kept.invite).await?,
        CloudJoined::Waiting(approval) => {
          let waiting = AcceptedInvite {
            approval: Some(approval.id),
            approval_expires_at: Some(Timestamp::from_unix_millis(approval.expires_at)),
            ..kept
          };
          self.keep_accepted(scheduled(waiting, now)).await?;
        }
      },
      Err(CloudError::InviteEnded | CloudError::JoinRefused) => self.forget_accepted(kept.invite).await?,
      // A first accept refused for want of a code leaves the invite open, listed by the cloud again. One an earlier
      // ask took a token for is listed there no more: it stays here, not asked on its own, until a sign-in with a code.
      Err(CloudError::TwoFactorOff | CloudError::TwoFactorSession) if first => {
        self.forget_accepted(kept.invite).await?
      }
      Err(CloudError::TwoFactorOff | CloudError::TwoFactorSession) => {
        self
          .keep_accepted(AcceptedInvite {
            next_check_at: None,
            ..kept
          })
          .await?;
      }
      // Kept: asked again on its own while its approval is open, and by the person.
      Err(_) => self.keep_accepted(scheduled(kept, now)).await?,
    }
    self.changed();
    outcome
  }

  /// One ask for `kept`: its server from the cloud's registry, then the join with the approval it waits for, while
  /// that is open at `now`.
  async fn ask(&self, kept: &AcceptedInvite, now: Timestamp) -> Result<Accepted, CloudError> {
    let audience = kept.server;
    let server = self
      .with_token(|client, token| async move { ended(client.server(&token, audience).await) })
      .await?
      .server;
    let (client, token, account) = self.signed_in().await?;
    let identity = CloudIdentity {
      cloud: &client,
      device: &token,
      account,
      invite: Some(kept.invite),
    };
    let approval = kept.approval.filter(|_| approval_open(kept, now));
    let joined = self
      .servers()
      .join_with_cloud(&server, identity, approval)
      .await
      .map_err(|error| ending(&error).unwrap_or_else(|| error.into()))?;
    Ok(Accepted {
      server,
      project: kept.project_name.clone(),
      joined,
    })
  }

  async fn keep_accepted(&self, kept: AcceptedInvite) -> Result<(), CloudError> {
    let workspace = self.inner.workspace.clone();
    Ok(background::run(move || workspace.db.lock().keep_accepted_invite(&kept)).await??)
  }

  async fn forget_accepted(&self, invite: InviteId) -> Result<(), CloudError> {
    let workspace = self.inner.workspace.clone();
    Ok(background::run(move || workspace.db.lock().forget_accepted_invite(invite)).await??)
  }
}

/// How a failed join ends this ask without asking again on its own: the cloud no longer issues tokens for the invite
/// (declined elsewhere, revoked or expired), the server has no invite for it, or the server refused this computer; or
/// the cloud refused a token without two-factor authentication.
fn ending(error: &SharingError) -> Option<CloudError> {
  let SharingError::Client(error) = error else {
    return None;
  };
  match error {
    ClientError::Cloud {
      code: CloudErrorCode::NotFound,
      ..
    } => Some(CloudError::InviteEnded),
    ClientError::Cloud {
      code: CloudErrorCode::MfaRequired,
      ..
    } => Some(CloudError::TwoFactorOff),
    ClientError::Cloud {
      code: CloudErrorCode::SessionWithoutMfa,
      ..
    } => Some(CloudError::TwoFactorSession),
    _ => match error.code()? {
      ErrorCode::NotInvited => Some(CloudError::InviteEnded),
      ErrorCode::ApprovalRefused | ErrorCode::Forbidden => Some(CloudError::JoinRefused),
      _ => None,
    },
  }
}
