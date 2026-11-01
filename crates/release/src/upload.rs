//! The upload's three steps against the cloud (`docs/cloud-api.md`, "Uploading a release"), each safe to run again:
//! a draft that exists already is taken up, a file declared again answers the parts the cloud has, and only the
//! missing ones are sent.

use orchester_api::{
  client::{ClientError, CloudClient},
  cloud::CloudErrorCode,
  releases::{FileResult, NewRelease, ReleaseVersion, Verification},
};
use orchester_core::secret::SecretValue;

use crate::{PublishError, ReleaseFile};

/// What creating the draft found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Draft {
  Created,
  /// The version existed already (`409 conflict`): a draft an earlier run left, taken up. A published or withdrawn
  /// release refuses the declarations that follow.
  Existing,
}

/// `POST /releases`: the release's draft.
pub async fn create_draft(
  client: &CloudClient,
  token: &SecretValue,
  release: &NewRelease,
) -> Result<Draft, PublishError> {
  match client.create_release(token, release).await {
    Ok(_) => Ok(Draft::Created),
    Err(ClientError::Cloud {
      code: CloudErrorCode::Conflict,
      ..
    }) => Ok(Draft::Existing),
    Err(error) => Err(PublishError::cloud(format!("creating the release {}", release.version))(error)),
  }
}

/// Declares `file` and sends the parts the cloud does not have yet; answers how many parts it sent.
pub async fn upload_file(
  client: &CloudClient,
  token: &SecretValue,
  version: &ReleaseVersion,
  file: &ReleaseFile,
) -> Result<usize, PublishError> {
  let name = file.name();
  let mut upload = client
    .declare_file(token, version, file.declared())
    .await
    .map_err(PublishError::cloud(format!("declaring {name}")))?;
  let parts = file.parts(&upload)?;
  let sent = parts.len();
  for (part, bytes) in parts {
    upload = client
      .upload_part(token, version, name, part, bytes.to_vec())
      .await
      .map_err(PublishError::cloud(format!("sending part {part} of {name}")))?;
  }
  let missing = upload.missing();
  if !missing.is_empty() {
    return Err(PublishError::PartsMissing {
      name: name.clone(),
      missing,
    });
  }
  Ok(sent)
}

/// `POST /releases/{version}/finalize`: the cloud's verdict, an error unless it accepted every file of `files`.
pub async fn finalize(
  client: &CloudClient,
  token: &SecretValue,
  version: &ReleaseVersion,
  files: &[ReleaseFile],
) -> Result<Verification, PublishError> {
  let verification = client
    .finalize_release(token, version)
    .await
    .map_err(PublishError::cloud(format!("finalizing the release {version}")))?;
  let every_file = files.iter().all(|file| {
    verification
      .files
      .iter()
      .any(|verdict| &verdict.name == file.name() && verdict.result == FileResult::Accepted)
  });
  if !verification.accepted() || !every_file {
    return Err(PublishError::Rejected(verification.files));
  }
  Ok(verification)
}
