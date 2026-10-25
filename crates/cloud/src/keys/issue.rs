use deadpool_postgres::GenericClient;
use orchester_api::{
  cloud::{CloudErrorCode, CloudPlatform, DeviceInfo, IdentityTokenIssued, IdentityTokenRequest, TokenPurpose},
  identity::{IdentityClaims, IdentityError, SigningKey, TokenSubject, VerifiedKeySet},
};
use orchester_core::id::{CloudAccountId, InviteId};
use uuid::Uuid;

use super::CLOCK_SKEW_MS;
use crate::{
  Cloud,
  accounts::{Authenticated, require_two_factor_session, two_factor_session},
  clock::seconds,
  db::{audit, time},
  error::CloudError,
};

impl Cloud {
  /// The newest published key set that has not expired and is not dated in the future, as the root key signed it, read
  /// with the caller's connection: an issuance holds one connection, never two (review 1, M4).
  pub(crate) async fn key_set(&self, client: &impl GenericClient) -> Result<String, CloudError> {
    let now = self.now();
    let row = client
      .query_opt(
        "SELECT compact FROM key_sets WHERE expires_at > $1 AND issued_at <= $2 ORDER BY issued_at DESC LIMIT 1",
        &[&time(now), &time(now + CLOCK_SKEW_MS)],
      )
      .await?
      .ok_or(CloudErrorCode::KeysUnavailable)?;
    Ok(row.try_get(0)?)
  }

  /// `POST /identity-tokens`: a token to join a team server, with purpose `admin` to sign in to the admin of a server
  /// linked here, or with purpose `code` to show with a code entered in the app. A token for linking comes only with
  /// `POST /servers`.
  pub(crate) async fn issue_identity_token(
    &self,
    auth: Authenticated,
    request: IdentityTokenRequest,
  ) -> Result<IdentityTokenIssued, CloudError> {
    let audience = Uuid::parse_str(request.audience.trim()).map_err(|_| CloudErrorCode::Invalid)?;
    let used = TokenUse::requested(&request)?;
    let client = self.client().await?;
    if let Some(invite) = request.invite {
      // Before the invite is looked at, so a refusal leaves it open (docs/cloud-api.md, "Two-factor authentication").
      require_two_factor_session(&client, auth.device).await?;
      crate::invites::accept_for(&client, auth, (invite, audience), self.now()).await?;
    }
    let linked = crate::servers::linked(&client, audience).await?.is_some();
    match request.purpose {
      Some(TokenPurpose::Admin) if !linked => return Err(CloudErrorCode::NotFound.into()),
      // A server not linked here may still be one an app joins with a token (row 42); linked, the account is
      // listed among the server's people on the account's next computer.
      None if linked => crate::servers::joined(&client, audience, auth, self.now()).await?,
      _ => {}
    }
    drop(client);
    self.issue_for(auth, audience.to_string(), used).await
  }

  /// A five-minute token for one team server, about the device's account, for the use `used` names, signed with the
  /// newest key the published key set lets sign now and naming the issuer that key set names: a team server holding
  /// it takes the token even while the cloud moves to another public URL (`docs/cloud-hosting.md`, "Moving the cloud").
  pub(crate) async fn issue_for(
    &self,
    auth: Authenticated,
    audience: String,
    used: TokenUse,
  ) -> Result<IdentityTokenIssued, CloudError> {
    self.inner.limits.identity_token(auth.device, self.now())?;
    let now = self.now();
    let client = self.inner.db.client().await?;
    let subject = self.subject(&client, auth, audience, used).await?;
    let (key, issuer) = self.signing_key(&client, now).await?;
    let claims = IdentityClaims::new(TokenSubject { issuer, ..subject }, seconds(now))?;
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

  /// Who the token is about, issued by this cloud's public URL until [`Self::issue_for`] names its key set's issuer.
  async fn subject(
    &self,
    client: &impl GenericClient,
    auth: Authenticated,
    audience: String,
    used: TokenUse,
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
      invite: used.invite.map(|invite| invite.to_string()),
      purpose: used.purpose,
      mfa: two_factor_session(client, auth.device).await?,
    })
  }

  /// The cloud signs only with a key a team server holding the newest key set would accept now; with the issuer that
  /// key set names.
  async fn signing_key(&self, client: &impl GenericClient, now: i64) -> Result<(SigningKey, String), CloudError> {
    let compact = self.key_set(client).await?;
    let verified = match VerifiedKeySet::verify(&compact, &self.inner.root, seconds(now)) {
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
    Ok((key, verified.statement().iss.clone()))
  }
}

/// What one identity token is for: its purpose and the invite it accepts.
pub(crate) struct TokenUse {
  purpose: Option<TokenPurpose>,
  invite: Option<InviteId>,
}

impl TokenUse {
  /// Linking the team server to the account of the admin who set it up (`POST /servers`).
  pub(crate) fn link() -> Self {
    Self {
      purpose: Some(TokenPurpose::Link),
      invite: None,
    }
  }

  /// The use a request asks for: an invite only to join, and never a link token, which only `POST /servers` gives. A
  /// code's token needs no more: its audience is the id of the server's key, which the app had the server prove it
  /// holds before asking.
  fn requested(request: &IdentityTokenRequest) -> Result<Self, CloudError> {
    let invite_allowed = request.invite.is_none() || request.purpose.is_none();
    if request.purpose == Some(TokenPurpose::Link) || !invite_allowed {
      return Err(CloudErrorCode::Invalid.into());
    }
    Ok(Self {
      purpose: request.purpose,
      invite: request.invite,
    })
  }
}
