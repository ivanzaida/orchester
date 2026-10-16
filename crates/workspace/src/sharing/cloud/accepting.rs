//! Accepting an invite through Orchester Cloud (`docs/cloud-api.md`, "Accepting an invite"): this computer reads the
//! invite's server from the cloud's registry, which holds the address and certificate fingerprint the server linked
//! with, and joins it with an identity token that names the invite, trusting only that certificate there, so there is
//! no check code to compare. The cloud lists an accepted invite no more but keeps issuing tokens for it to this
//! account, so this computer keeps the invite until the join finished (N1 security review 1, M2): a join that failed is
//! asked again, and one that waits for approval is asked again with that approval until it is approved or refused.

use orchester_api::{
  ErrorCode,
  client::ClientError,
  cloud::{CloudErrorCode, CloudInvite, CloudServer},
};
use orchester_core::{id::InviteId, time::Timestamp};
use orchester_store::AcceptedInvite;

use super::{Cloud, CloudError, invites::ended};
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
    let kept = AcceptedInvite {
      invite: invite.id,
      server: invite.server.id,
      server_name: invite.server.name.clone(),
      project_name: invite.project.as_ref().map(|project| project.name.clone()),
      expires_at: Timestamp::from_unix_millis(invite.expires_at),
      approval: None,
      accepted_at: Timestamp::now(),
    };
    self.keep_accepted(kept.clone()).await?;
    self.join_accepted(kept).await
  }

  /// Asks again for `invite`, accepted here before: after a join that failed, or while it waits for approval.
  pub async fn finish_accept(&self, invite: InviteId) -> Result<Accepted, CloudError> {
    let kept = self
      .accepted_invites()
      .await?
      .into_iter()
      .find(|kept| kept.invite == invite)
      .ok_or(CloudError::InviteEnded)?;
    self.join_accepted(kept).await
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

  async fn join_accepted(&self, kept: AcceptedInvite) -> Result<Accepted, CloudError> {
    let invite = kept.invite;
    let registered = self
      .with_token(|client, token| async move { ended(client.server(&token, kept.server).await) })
      .await;
    let server = match registered {
      Ok(registered) => registered.server,
      Err(CloudError::InviteEnded) => {
        self.forget_accepted(invite).await?;
        self.changed();
        return Err(CloudError::InviteEnded);
      }
      Err(error) => return Err(error),
    };
    let (client, token) = self.signed_in().await?;
    let identity = CloudIdentity {
      cloud: &client,
      device: &token,
      invite: Some(invite),
    };
    let joined = self.servers().join_with_cloud(&server, identity, kept.approval).await;
    let outcome = match joined {
      Ok(CloudJoined::Joined(id)) => {
        self.forget_accepted(invite).await?;
        Ok(CloudJoined::Joined(id))
      }
      Ok(CloudJoined::Waiting(approval)) => {
        let waiting = AcceptedInvite {
          approval: Some(approval.id),
          ..kept.clone()
        };
        self.keep_accepted(waiting).await?;
        Ok(CloudJoined::Waiting(approval))
      }
      Err(error) => match ending(&error) {
        Some(ended) => {
          self.forget_accepted(invite).await?;
          Err(ended)
        }
        // Kept: the person asks again.
        None => Err(error.into()),
      },
    };
    self.changed();
    outcome.map(|joined| Accepted {
      server,
      project: kept.project_name,
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

/// How a failed join ends the invite for good, so it is not asked again: the cloud no longer issues tokens for it
/// (declined elsewhere, revoked or expired), the server has no invite for it, or the server refused this computer.
fn ending(error: &SharingError) -> Option<CloudError> {
  let SharingError::Client(error) = error else {
    return None;
  };
  match error {
    ClientError::Cloud {
      code: CloudErrorCode::NotFound,
      ..
    } => Some(CloudError::InviteEnded),
    _ => match error.code()? {
      ErrorCode::NotInvited => Some(CloudError::InviteEnded),
      ErrorCode::ApprovalRefused | ErrorCode::Forbidden => Some(CloudError::JoinRefused),
      _ => None,
    },
  }
}
