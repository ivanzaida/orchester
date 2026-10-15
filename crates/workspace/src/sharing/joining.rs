//! Joining a team server from this computer (`docs/architecture/server.md`, "Accounts and tokens", rows 42–45, 50):
//! with a code (setup, invite, account or pairing) or with an Orchester Cloud identity token. The server issues this
//! computer one device token, kept in the keychain and used by every local project linked to a project there. The
//! server's certificate is checked before anything is sent: a pinned server that presents another certificate gets
//! no code and no token.

use orchester_api::{
  Approval, CloudJoin, CloudJoinAnswer, CodeJoin, Credential, Joined, PairingCode,
  client::{ClientError, CloudClient, ServerAddress},
  cloud::CloudServer,
};
use orchester_core::{
  id::{ApprovalId, ServerId},
  secret::SecretValue,
  time::Timestamp,
};
use orchester_store::StoredServer;

use super::{ServerEntry, Servers, SharingError, account::account, device, trust::Trust};
use crate::background;

/// What a code redeems; `username` and `display_name` only when the code makes an account (setup and invite codes).
pub struct CodeRequest {
  pub code: Credential,
  pub username: Option<String>,
  pub display_name: Option<String>,
}

/// How a cloud join ended.
#[derive(Debug)]
pub enum CloudJoined {
  /// This computer joined the server, now in the list with this id.
  Joined(ServerId),
  /// Join approval holds the join until one of the account's devices or an admin approves it; ask again with the
  /// approval's id.
  Waiting(Approval),
}

/// What a cloud join needs from Orchester Cloud: its client and this computer's cloud device token.
pub struct CloudIdentity<'a> {
  pub cloud: &'a CloudClient,
  pub device: &'a SecretValue,
}

impl Servers {
  /// Joins the team server at `address`, trusted as `trust`, with a code, and keeps the device token. The server is
  /// added to the list when it is not in it yet.
  pub async fn join_with_code(
    &self,
    address: &ServerAddress,
    trust: Trust,
    request: CodeRequest,
  ) -> Result<ServerId, SharingError> {
    let client = trust.client(address.clone())?;
    // The certificate is checked by this first request, before the code is sent.
    let name = client.server_info().await.map_err(|error| trust.refusal(error))?.name;
    let joined = client
      .join_with_code(&CodeJoin {
        code: request.code,
        device: device::this_computer(),
        username: request.username,
        display_name: request.display_name,
      })
      .await?;
    self.keep_join(address, trust, name, joined).await
  }

  /// Joins `server`, as Orchester Cloud names it, with a cloud identity: at the address the cloud has for it and
  /// trusting only the certificate the cloud has for it, so an identity token for that server reaches no other. The
  /// cloud gives the token and its current key set, which the server may not have, and the server verifies both
  /// without the internet. `approval` finishes a join that waited for approval.
  pub async fn join_with_cloud(
    &self,
    server: &CloudServer,
    identity: CloudIdentity<'_>,
    approval: Option<ApprovalId>,
  ) -> Result<CloudJoined, SharingError> {
    let address = ServerAddress::parse(&server.address).map_err(ClientError::Address)?;
    let trust = Trust::named_by_cloud(server)?;
    let client = trust.client(address.clone())?;
    // The certificate is checked by this first request, before anything of the cloud is sent.
    let name = client.server_info().await.map_err(|error| trust.refusal(error))?.name;
    let key_set = identity.cloud.key_set().await?.key_set;
    let issued = identity.cloud.identity_token(identity.device, server.id).await?;
    let request = CloudJoin {
      identity_token: issued.identity_token,
      key_set: Some(key_set),
      device: device::this_computer(),
      approval,
    };
    match client.join_with_cloud(&request).await? {
      CloudJoinAnswer::Joined(joined) => Ok(CloudJoined::Joined(
        self.keep_join(&address, trust, name, joined).await?,
      )),
      CloudJoinAnswer::Waiting(approval) => Ok(CloudJoined::Waiting(approval)),
    }
  }

  /// A code for this person's next computer (*Add a computer*), valid 10 minutes, shown once.
  pub async fn pairing_code(&self, server: &ServerEntry) -> Result<PairingCode, SharingError> {
    self
      .with_token(server, |client, token| async move { client.pairing_code(&token).await })
      .await
  }

  /// Keeps what a join gave: the server in the list with its trust and name, the device token in the keychain and
  /// the account.
  async fn keep_join(
    &self,
    address: &ServerAddress,
    trust: Trust,
    name: String,
    joined: Joined,
  ) -> Result<ServerId, SharingError> {
    // The server issues `orc_` and 64 hex characters; a blank answer is not a token.
    let token =
      SecretValue::new(joined.token.expose().to_owned()).map_err(|_| SharingError::InvalidAnswer("device token"))?;
    let (pin, plain_http) = trust.columns();
    let (workspace, url, user) = (
      self.inner.workspace.clone(),
      address.as_str().to_owned(),
      account(&joined.user),
    );
    let id = background::run(move || -> Result<ServerId, SharingError> {
      let known = workspace.servers()?.into_iter().find(|server| server.url == url);
      let id = match known {
        Some(server) => {
          workspace.set_server_trust(server.id, pin.as_deref(), plain_http)?;
          workspace.set_server_name(server.id, &name)?;
          server.id
        }
        None => {
          let server = StoredServer {
            id: ServerId::generate(),
            url,
            name,
            plain_http_allowed: plain_http,
            certificate_pin: pin,
            added_at: Timestamp::now(),
            account: None,
          };
          workspace.add_server(&server)?;
          server.id
        }
      };
      workspace.set_server_account(id, Some(&user))?;
      Ok(id)
    })
    .await??;
    self.store_token(id, token).await?;
    self.reached(id);
    Ok(id)
  }
}
