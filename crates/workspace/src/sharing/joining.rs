//! Joining a team server from this computer (`docs/architecture/server.md`, "Accounts and tokens", rows 42–45, 50):
//! with a code (setup, invite, account or pairing) or with an Orchester Cloud identity token. The server issues this
//! computer one device token, kept in the keychain and used by every local project linked to a project there. The
//! server's certificate is checked before anything is sent: a pinned server that presents another certificate gets
//! no code and no token.

use orchester_api::{
  Approval, CloudJoin, CloudJoinAnswer, CodeJoin, Credential, DeviceKind, Joined, PairingCode,
  client::{Client, ClientError, CloudClient, ServerAddress},
  cloud::CloudServer,
};
use orchester_core::{
  id::{ApprovalId, InviteId, ServerId, TeamServerId},
  secret::SecretValue,
  time::Timestamp,
};
use orchester_store::{JoinedWith, StoredServer};

use super::{ServerEntry, Servers, SharingError, account::account, device, trust::Trust};
use crate::background;

/// What a code redeems; `username` and `display_name` only when the code makes an account (setup and invite codes).
/// `proof` shows the person signed in with two-factor authentication, which every code but the setup code needs.
pub struct CodeRequest {
  pub code: Credential,
  pub username: Option<String>,
  pub display_name: Option<String>,
  pub proof: Option<CodeProof>,
}

/// This computer's Orchester Cloud identity token for the server, whose `mfa` claim shows it signed in with two-factor
/// authentication, and the key set that vouches for it, for a server that cannot reach the cloud
/// (`docs/architecture/server.md`, "Codes").
pub struct CodeProof {
  pub identity_token: Credential,
  pub key_set: Option<String>,
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

/// What a cloud join needs from Orchester Cloud: its client, this computer's cloud device token, and the invite the
/// identity token accepts, when the join answers one.
pub struct CloudIdentity<'a> {
  pub cloud: &'a CloudClient,
  pub device: &'a SecretValue,
  pub invite: Option<InviteId>,
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
    let (identity_token, key_set) = request
      .proof
      .map_or((None, None), |proof| (Some(proof.identity_token), proof.key_set));
    let joined = client
      .join_with_code(&CodeJoin {
        code: request.code,
        device: device::this_computer(),
        username: request.username,
        display_name: request.display_name,
        kind: DeviceKind::App,
        identity_token,
        key_set,
      })
      .await?;
    self.keep_join(address, (trust, JoinedWith::Code), name, joined).await
  }

  /// What a code needs from the cloud for the server at `address`: `None` when the server pins no cloud root and so
  /// cannot check it (it then takes only its setup code). Otherwise the account must have two-factor authentication on
  /// and this computer must have signed in with it, checked before the code is sent ([`SharingError::TwoFactorOff`],
  /// [`SharingError::TwoFactorSession`]). The server's certificate is checked first, before anything of the cloud.
  pub async fn code_proof(
    &self,
    address: &ServerAddress,
    trust: Trust,
    identity: CloudIdentity<'_>,
  ) -> Result<Option<CodeProof>, SharingError> {
    let client = trust.client(address.clone())?;
    let info = client.server_info().await.map_err(|error| trust.refusal(error))?;
    let Some(audience) = info.cloud.and_then(|cloud| cloud.server.parse::<TeamServerId>().ok()) else {
      return Ok(None);
    };
    let two_factor = identity.cloud.me(identity.device).await?.two_factor;
    if !two_factor.enabled {
      return Err(SharingError::TwoFactorOff);
    }
    if !two_factor.this_session {
      return Err(SharingError::TwoFactorSession);
    }
    let key_set = identity.cloud.key_set().await?.key_set;
    let issued = identity.cloud.identity_token(identity.device, audience, None).await?;
    Ok(Some(CodeProof {
      identity_token: issued.identity_token,
      key_set: Some(key_set),
    }))
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
    let (address, trust, client, name) = Self::reach_named(server).await?;
    let key_set = identity.cloud.key_set().await?.key_set;
    let issued = identity
      .cloud
      .identity_token(identity.device, server.id, identity.invite)
      .await?;
    let request = CloudJoin {
      identity_token: issued.identity_token,
      key_set: Some(key_set),
      device: device::this_computer(),
      approval,
      kind: DeviceKind::App,
    };
    self
      .finish_cloud_join(server, (&address, trust, &client, name), &request)
      .await
  }

  /// The client of `server` at the address the cloud names, trusting only the certificate it names, and the name the
  /// server gives itself: that first request checks the certificate before anything of the cloud is sent.
  async fn reach_named(server: &CloudServer) -> Result<(ServerAddress, Trust, Client, String), SharingError> {
    let address = ServerAddress::parse(&server.address).map_err(ClientError::Address)?;
    let trust = Trust::named_by_cloud(server)?;
    let client = trust.client(address.clone())?;
    let name = client.server_info().await.map_err(|error| trust.refusal(error))?.name;
    Ok((address, trust, client, name))
  }

  async fn finish_cloud_join(
    &self,
    server: &CloudServer,
    (address, trust, client, name): (&ServerAddress, Trust, &Client, String),
    request: &CloudJoin,
  ) -> Result<CloudJoined, SharingError> {
    match client.join_with_cloud(request).await? {
      CloudJoinAnswer::Joined(joined) => Ok(CloudJoined::Joined(
        self
          .keep_join(address, (trust, JoinedWith::Cloud(server.id)), name, joined)
          .await?,
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
    (trust, joined_with): (Trust, JoinedWith),
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
          workspace.db.lock().set_server_joined_with(server.id, joined_with)?;
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
            joined_with: Some(joined_with),
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
