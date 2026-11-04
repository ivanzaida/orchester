//! The routes of authorized work on one team project (`docs/cloud-api.md`, "Team authority P4 work"): the execution
//! bundle a work start names and the files it pins, read by their hashes in the session the claim executes under, and
//! the exact heartbeat that renews that session's claims while their work runs.

use orchester_core::{blob::ContentHash, run::bundle::MAX_BUNDLE_FILE_BYTES};
use reqwest::Method;

use super::{Authority, AuthorityCallError, answered};
use crate::{
  client::ClientError,
  v2::{ExactHeartbeat, ExecutionBundle, Renewed},
};

impl Authority<'_> {
  /// `GET …/bundles/{sha256}`: the bundle as the server built it. `409 claim_lost` once no claim it was built for is
  /// the caller's in this session, and for a hash the workspace does not have. The caller checks it has the hash it
  /// asked for.
  pub async fn bundle(&self, sha256: &ContentHash) -> Result<ExecutionBundle, AuthorityCallError> {
    self.get(&format!("/bundles/{sha256}")).await
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
