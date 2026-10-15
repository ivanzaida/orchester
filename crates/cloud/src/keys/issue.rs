use deadpool_postgres::GenericClient;
use orchester_api::{
  cloud::{CloudErrorCode, CloudPlatform, DeviceInfo, IdentityTokenIssued, IdentityTokenRequest},
  identity::{IdentityClaims, IdentityError, SigningKey, TokenSubject, VerifiedKeySet},
};
use orchester_core::id::CloudAccountId;
use uuid::Uuid;

use crate::{
  Cloud,
  accounts::Authenticated,
  clock::seconds,
  db::{audit, time},
  error::CloudError,
};

impl Cloud {
  /// The newest published key set that has not expired, as the root key signed it.
  pub(crate) async fn key_set(&self) -> Result<String, CloudError> {
    let row = self
      .inner
      .db
      .client()
      .await?
      .query_opt(
        "SELECT compact FROM key_sets WHERE expires_at > $1 ORDER BY issued_at DESC LIMIT 1",
        &[&time(self.now())],
      )
      .await?
      .ok_or(CloudErrorCode::KeysUnavailable)?;
    Ok(row.try_get(0)?)
  }

  /// A five-minute token for one team server, about the device's account, signed with the newest key the published
  /// key set lets sign now.
  pub(crate) async fn issue_identity_token(
    &self,
    auth: Authenticated,
    request: IdentityTokenRequest,
  ) -> Result<IdentityTokenIssued, CloudError> {
    // A team server's id; the registry that knows them comes with linking (C1, not built yet).
    let audience = Uuid::parse_str(request.audience.trim()).map_err(|_| CloudErrorCode::Invalid)?;
    self.inner.limits.identity_token(auth.device, self.now())?;
    let now = self.now();
    let client = self.inner.db.client().await?;
    let subject = self.subject(&client, auth, audience.to_string()).await?;
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
    })
  }

  /// The cloud signs only with a key a team server holding the newest key set would accept now.
  async fn signing_key(&self, client: &impl GenericClient, now: i64) -> Result<SigningKey, CloudError> {
    let compact = self.key_set().await?;
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
