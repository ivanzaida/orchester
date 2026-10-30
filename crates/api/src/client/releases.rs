//! What the app and CI ask Orchester Cloud about app releases (`docs/cloud-api.md`, "App releases"): the anonymous
//! update check and download, and CI's upload with a release token (scope `releases:write`).

use std::time::Duration;

use orchester_core::secret::SecretValue;
use reqwest::{
  Method, Response, StatusCode,
  header::{CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE},
};

use super::{
  ClientError, CloudClient, bearer,
  cloud::{answered, call},
};
use crate::releases::{
  DeclaredFile, FileUpload, NewRelease, OfferedRelease, ReleaseFileName, ReleaseResponse, ReleaseSummary,
  ReleaseVersion, UpdateCheck, UpdateCheckAnswer, Verification,
};

/// How long a whole download may take; a stalled one ends sooner, at the client's read timeout.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
/// How long sending one part of a file may take.
const PART_TIMEOUT: Duration = Duration::from_secs(5 * 60);

impl CloudClient {
  /// `GET /updates/check`: the release offered to this install, or `None` when nothing newer is. No sign-in.
  pub async fn check_for_update(&self, check: &UpdateCheck) -> Result<Option<OfferedRelease>, ClientError> {
    let request = self.request_with_query(Method::GET, "/updates/check", check.query())?;
    call::<UpdateCheckAnswer>(request).await.map(|answer| answer.update)
  }

  /// `GET /releases/{version}/files/{name}` from byte `from` (a `Range` when `from` is not 0, to resume). No sign-in.
  pub async fn download(
    &self,
    version: &ReleaseVersion,
    name: &ReleaseFileName,
    from: u64,
  ) -> Result<ReleaseDownload, ClientError> {
    let mut request = self
      .request(Method::GET, &file_path(version, name))?
      .timeout(DOWNLOAD_TIMEOUT);
    if from > 0 {
      request = request.header(RANGE, format!("bytes={from}-"));
    }
    ReleaseDownload::new(answered(request).await?)
  }

  /// `POST /releases`: a new draft release, with a release token.
  pub async fn create_release(&self, token: &SecretValue, release: &NewRelease) -> Result<ReleaseSummary, ClientError> {
    let request = bearer(self.json(Method::POST, "/releases", release)?, token);
    call::<ReleaseResponse>(request).await.map(|answer| answer.release)
  }

  /// `POST /releases/{version}/files`: declares one file of the draft `version`, or reads its upload as it stands to
  /// resume it.
  pub async fn declare_file(
    &self,
    token: &SecretValue,
    version: &ReleaseVersion,
    file: &DeclaredFile,
  ) -> Result<FileUpload, ClientError> {
    let path = format!("/releases/{version}/files");
    call(bearer(self.json(Method::POST, &path, file)?, token)).await
  }

  /// `PUT /releases/{version}/files/{name}/parts/{part}`: one part of a declared file, `part_size` bytes but for the
  /// last. Sending a part again replaces it.
  pub async fn upload_part(
    &self,
    token: &SecretValue,
    version: &ReleaseVersion,
    name: &ReleaseFileName,
    part: u32,
    bytes: Vec<u8>,
  ) -> Result<FileUpload, ClientError> {
    let path = format!("{}/parts/{part}", file_path(version, name));
    let request = self
      .request(Method::PUT, &path)?
      .timeout(PART_TIMEOUT)
      .header(CONTENT_TYPE, "application/octet-stream")
      .body(bytes);
    call(bearer(request, token)).await
  }

  /// `POST /releases/{version}/finalize`: checks every declared file and publishes the release when all pass.
  pub async fn finalize_release(
    &self,
    token: &SecretValue,
    version: &ReleaseVersion,
  ) -> Result<Verification, ClientError> {
    let path = format!("/releases/{version}/finalize");
    call(bearer(self.request(Method::POST, &path)?, token)).await
  }
}

/// A release file's path; both parts are limited to characters a URL path takes as they are.
fn file_path(version: &ReleaseVersion, name: &ReleaseFileName) -> String {
  format!("/releases/{version}/files/{name}")
}

/// A download in progress: the file's bytes from [`Self::start`] on, read a chunk at a time.
#[derive(Debug)]
pub struct ReleaseDownload {
  response: Response,
  start: u64,
  total: u64,
}

impl ReleaseDownload {
  fn new(response: Response) -> Result<Self, ClientError> {
    let status = response.status();
    let headers = response.headers();
    let text = |name| headers.get(name).and_then(|value| value.to_str().ok());
    let span = match status {
      StatusCode::OK => text(CONTENT_LENGTH)
        .and_then(|length| length.parse().ok())
        .map(|total| (0, total)),
      StatusCode::PARTIAL_CONTENT => text(CONTENT_RANGE).and_then(content_range),
      _ => None,
    };
    let (start, total) = span.ok_or(ClientError::NotOrchester {
      status: status.as_u16(),
    })?;
    Ok(Self { response, start, total })
  }

  /// The byte the answer starts at: the asked one for a `206`, or 0 when the cloud sent the whole file (`200`).
  pub fn start(&self) -> u64 {
    self.start
  }

  /// The whole file's size.
  pub fn total(&self) -> u64 {
    self.total
  }

  /// The next bytes of the answer; `None` at its end.
  pub async fn chunk(&mut self) -> Result<Option<Vec<u8>>, ClientError> {
    self
      .response
      .chunk()
      .await
      .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
      .map_err(|error| ClientError::transport(&error))
  }
}

/// The start and the total of `Content-Range: bytes <start>-<end>/<total>`.
fn content_range(text: &str) -> Option<(u64, u64)> {
  let (range, total) = text.strip_prefix("bytes ")?.split_once('/')?;
  let (start, end) = range.split_once('-')?;
  let (start, end, total): (u64, u64, u64) = (start.parse().ok()?, end.parse().ok()?, total.parse().ok()?);
  (start <= end && end < total).then_some((start, total))
}
