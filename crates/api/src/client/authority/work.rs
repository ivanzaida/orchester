//! The routes of authorized work on one team project (`docs/cloud-api.md`, "Team authority P4 work"): the execution
//! bundle a work start names and the files it pins, read by their hashes in the session the claim executes under, and
//! the exact heartbeat that renews that session's claims while their work runs.

use orchester_core::{blob::ContentHash, run::bundle::MAX_BUNDLE_FILE_BYTES};
use reqwest::Method;
use serde_json::Value;
use thiserror::Error;

use super::{Authority, AuthorityCallError, answered};
use crate::{
  client::ClientError,
  v2::{EXECUTION_BUNDLE_SCHEMA, ExactHeartbeat, ExecutionBundle, Renewed},
};

/// Why [`Authority::bundle`] gave no bundle. A bundle this client cannot read is told apart from a call that failed:
/// reading it again gives the same bundle, so waiting does not help.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum BundleReadError {
  /// The call failed or was refused, before any bundle was read.
  #[error(transparent)]
  Call(#[from] AuthorityCallError),
  /// The server built the bundle in another schema than [`EXECUTION_BUNDLE_SCHEMA`], the one this client reads.
  #[error("the bundle is built in schema {built}, and this client reads schema {EXECUTION_BUNDLE_SCHEMA}")]
  Schema { built: u32 },
  /// The server answered JSON that is not an execution bundle of the schema this client reads.
  #[error("the server's answer is not an execution bundle (HTTP {status})")]
  Unreadable { status: u16 },
}

impl Authority<'_> {
  /// `GET …/bundles/{sha256}`: the bundle as the server built it. `409 claim_lost` once no claim it was built for is
  /// the caller's in this session, and for a hash the workspace does not have. The caller checks it has the hash it
  /// asked for. A body that is not JSON is not an Orchester server's answer ([`ClientError::NotOrchester`]); JSON that
  /// is not a bundle of this client's schema is a [`BundleReadError::Schema`] or [`BundleReadError::Unreadable`].
  pub async fn bundle(&self, sha256: &ContentHash) -> Result<ExecutionBundle, BundleReadError> {
    let (status, bytes) = self.bundle_answer(sha256).await?;
    serde_json::from_slice(&bytes).map_err(|_| match serde_json::from_slice::<Value>(&bytes) {
      Ok(body) => match body["schema_version"]
        .as_u64()
        .and_then(|built| u32::try_from(built).ok())
      {
        Some(built) if built != EXECUTION_BUNDLE_SCHEMA => BundleReadError::Schema { built },
        _ => BundleReadError::Unreadable { status },
      },
      Err(_) => BundleReadError::Call(ClientError::NotOrchester { status }.into()),
    })
  }

  async fn bundle_answer(&self, sha256: &ContentHash) -> Result<(u16, Vec<u8>), AuthorityCallError> {
    let response = answered(self.request(Method::GET, &format!("/bundles/{sha256}"))?).await?;
    let status = response.status().as_u16();
    let bytes = response.bytes().await.map_err(|error| ClientError::transport(&error))?;
    Ok((status, bytes.to_vec()))
  }

  /// `GET …/bundles/{sha256}/files/{file}`: the raw bytes of a file the bundle pins, at most
  /// [`MAX_BUNDLE_FILE_BYTES`]; `404 not_found` for a file it does not pin. The caller checks them against the bundle
  /// (`ExecutionBundle::check_file`).
  pub async fn bundle_file(&self, bundle: &ContentHash, file: &ContentHash) -> Result<Vec<u8>, AuthorityCallError> {
    let response = answered(self.request(Method::GET, &format!("/bundles/{bundle}/files/{file}"))?).await?;
    let status = response.status().as_u16();
    let bytes = response.bytes().await.map_err(|error| ClientError::transport(&error))?;
    // More than the contract's most is not an Orchester server's answer.
    if !u64::try_from(bytes.len()).is_ok_and(|size| size <= MAX_BUNDLE_FILE_BYTES) {
      return Err(ClientError::NotOrchester { status }.into());
    }
    Ok(bytes.to_vec())
  }

  /// `POST …/heartbeat`: renews the claims `heartbeat` names, at most
  /// [`MAX_HEARTBEAT_AUTHORITIES`](crate::v2::MAX_HEARTBEAT_AUTHORITIES), while their work runs.
  pub async fn heartbeat(&self, heartbeat: &ExactHeartbeat) -> Result<Renewed, AuthorityCallError> {
    self.query("/heartbeat", heartbeat).await
  }
}
