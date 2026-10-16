use deadpool_postgres::GenericClient;
use orchester_api::{
  cloud::{CloudErrorCode, CloudPlatform, DeviceInfo, IdentityTokenIssued, IdentityTokenRequest, TokenPurpose},
  identity::{IdentityClaims, IdentityError, SigningKey, TokenSubject, VerifiedKeySet},
};
use orchester_core::id::{CloudAccountId, InviteId};
use uuid::Uuid;

use crate::{
  Cloud,
  accounts::Authenticated,
  clock::seconds,
  db::{audit, time},
  error::CloudError,
};

impl Cloud {
  /// The newest published key set that has not expired, as the root key signed it, read with the caller's
  /// connection: an issuance holds one connection, never two (review 1, M4).
  pub(crate) async fn key_set(&self, client: &impl GenericClient) -> Result<String, CloudError> {
    let row = client
      .query_opt(
        "SELECT compact FROM key_sets WHERE expires_at > $1 ORDER BY issued_at DESC LIMIT 1",
        &[&time(self.now())],
      )
      .await?
      .ok_or(CloudErrorCode::KeysUnavailable)?;
    Ok(row.try_get(0)?)
  }

  /// `POST /identity-tokens`: a token to join a team server, or with purpose `admin` to sign in to the admin of a
  /// server linked here. A token for linking comes only with `POST /servers`.
  pub(crate) async fn issue_identity_token(
    &self,
    auth: Authenticated,
    request: IdentityTokenRequest,
  ) -> Result<IdentityTokenIssued, CloudError> {
    let audience = Uuid::parse_str(request.audience.trim()).map_err(|_| CloudErrorCode::Invalid)?;
    let client = self.client().await?;
    if let Some(invite) = request.invite {
      if request.purpose.is_some() {
        return Err(CloudErrorCode::Invalid.into());
      }
      crate::invites::accept_for(&client, auth, (invite, audience), self.now()).await?;
    }
    let linked = crate::servers::linked(&client, audience).await?.is_some();
    match request.purpose {
      Some(TokenPurpose::Link) => return Err(CloudErrorCode::Invalid.into()),
      Some(TokenPurpose::Admin) if !linked => return Err(CloudErrorCode::NotFound.into()),
      // A server not linked here may still be one an app joins with a token (row 42); linked, the account is
      // listed among the server's people on the account's next computer.
      None if linked => crate::servers::joined(&client, audience, auth, self.now()).await?,
      _ => {}
    }
    drop(client);
    self
      .issue_for(auth, audience.to_string(), (request.purpose, request.invite))
      .await
  }

  /// A five-minute token for one team server, about the device's account, for `purpose` and accepting `invite`, signed
  /// with the newest key the published key set lets sign now.
  pub(crate) async fn issue_for(
    &self,
    auth: Authenticated,
    audience: String,
    (purpose, invite): (Option<TokenPurpose>, Option<InviteId>),
  ) -> Result<IdentityTokenIssued, CloudError> {
    self.inner.limits.identity_token(auth.device, self.now())?;
    let now = self.now();
    let client = self.inner.db.client().await?;
    let subject = self.subject(&client, auth, audience, (purpose, invite)).await?;
    let key = self.signing_key(&client, now).await?;
    let claims = IdentityClaims::new(subject, seconds(now))?;
    let token = key.sign_token(&claims)?;
    audit(
      &client,
      Some(auth.account),
      "identity_token_issued",
      &claims.aud,
      None,
      now,
    )
    .await?;
    Ok(IdentityTokenIssued {
      identity_token: token,
      expires_at: claims.exp,
    })
  }

  async fn subject(
    &self,
    client: &impl GenericClient,
    auth: Authenticated,
    audience: String,
    (purpose, invite): (Option<TokenPurpose>, Option<InviteId>),
  ) -> Result<TokenSubject, CloudError> {
    let row = client
      .query_opt(
        "SELECT a.id, a.email, a.display_name, d.name, d.platform FROM devices d JOIN accounts a ON a.id = d.account
         WHERE d.id = $1 AND d.revoked_at IS NULL AND a.confirmed_at IS NOT NULL",
        &[&auth.device.as_uuid()],
      )
      .await?
      .ok_or(CloudErrorCode::Unauthorized)?;
    Ok(TokenSubject {
      issuer: self.inner.config.public_url.clone(),
      audience,
      account: CloudAccountId::from_uuid(row.try_get(0)?),
      email: row.try_get(1)?,
      name: row.try_get(2)?,
      device: DeviceInfo {
        name: row.try_get(3)?,
        platform: CloudPlatform::parse(row.try_get(4)?).ok_or(CloudError::StoredData)?,
      },
      invite: invite.map(|invite| invite.to_string()),
      purpose,
    })
  }

  /// The cloud signs only with a key a team server holding the newest key set would accept now.
  async fn signing_key(&self, client: &impl GenericClient, now: i64) -> Result<SigningKey, CloudError> {
    let compact = self.key_set(client).await?;
    let verified = match VerifiedKeySet::verify(&compact, &self.inner.config.root_public_key, seconds(now)) {
      Ok(verified) => verified,
      Err(IdentityError::KeySetExpired | IdentityError::NotYetValid) => {
        return Err(CloudErrorCode::KeysUnavailable.into());
      }
      Err(error) => return Err(error.into()),
    };
    let at = seconds(now);
    let listed = verified
      .statement()
      .keys
      .iter()
      .filter(|key| key.not_before <= at && at < key.signs_until && at < key.listed_until)
      .max_by_key(|key| key.not_before)
      .ok_or(CloudErrorCode::KeysUnavailable)?;
    let row = client
      .query_one(
        "SELECT sealed_private_key FROM signing_keys WHERE kid = $1",
        &[&listed.kid],
      )
      .await?;
    let private = self.inner.sealing.open(&listed.kid, row.try_get(0)?)?;
    let key = SigningKey::new(&private)?;
    if key.kid() != listed.kid {
      return Err(CloudError::StoredData);
    }
    Ok(key)
  }
}
