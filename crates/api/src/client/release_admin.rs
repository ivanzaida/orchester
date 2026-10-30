//! What the cloud's admin asks about app releases (`docs/cloud-api.md`, "Release admin"): the device token of an
//! Orchester staff account's two-factor session.

use orchester_core::secret::SecretValue;
use reqwest::Method;

use super::{
  ClientError, CloudClient, bearer,
  cloud::{call, send},
};
use crate::releases::{
  PercentChange, ReleaseChanges, ReleaseDetails, ReleaseDetailsResponse, ReleaseResponse, ReleaseSummary,
  ReleaseVersion, ReleasesResponse, RolloutPercent,
};

impl CloudClient {
  /// `GET /admin/releases`: every release, newest version first.
  pub async fn admin_releases(&self, device: &SecretValue) -> Result<Vec<ReleaseSummary>, ClientError> {
    let request = bearer(self.request(Method::GET, "/admin/releases")?, device);
    call::<ReleasesResponse>(request).await.map(|answer| answer.releases)
  }

  /// `GET /admin/releases/{version}`: one release with its files, adoption and history.
  pub async fn admin_release(
    &self,
    device: &SecretValue,
    version: &ReleaseVersion,
  ) -> Result<ReleaseDetails, ClientError> {
    let request = bearer(self.request(Method::GET, &release_path(version))?, device);
    call::<ReleaseDetailsResponse>(request)
      .await
      .map(|answer| answer.release)
  }

  /// `POST /admin/releases/{version}/promote`: puts a Beta release on Stable for `percent` of installs.
  pub async fn promote_release(
    &self,
    device: &SecretValue,
    version: &ReleaseVersion,
    percent: RolloutPercent,
  ) -> Result<ReleaseSummary, ClientError> {
    let path = format!("{}/promote", release_path(version));
    let request = self.json(Method::POST, &path, &PercentChange { percent })?;
    changed(bearer(request, device)).await
  }

  /// `PUT /admin/releases/{version}/rollout`: a Stable release's new percentage.
  pub async fn set_rollout(
    &self,
    device: &SecretValue,
    version: &ReleaseVersion,
    percent: RolloutPercent,
  ) -> Result<ReleaseSummary, ClientError> {
    let path = format!("{}/rollout", release_path(version));
    let request = self.json(Method::PUT, &path, &PercentChange { percent })?;
    changed(bearer(request, device)).await
  }

  /// `POST /admin/releases/{version}/pause`: a Stable release is offered to no Stable install until it is resumed.
  pub async fn pause_release(
    &self,
    device: &SecretValue,
    version: &ReleaseVersion,
  ) -> Result<ReleaseSummary, ClientError> {
    self.move_release(device, version, "pause").await
  }

  /// `POST /admin/releases/{version}/resume`.
  pub async fn resume_release(
    &self,
    device: &SecretValue,
    version: &ReleaseVersion,
  ) -> Result<ReleaseSummary, ClientError> {
    self.move_release(device, version, "resume").await
  }

  /// `POST /admin/releases/{version}/withdraw`: the release is offered to nobody and its files are no longer served.
  pub async fn withdraw_release(
    &self,
    device: &SecretValue,
    version: &ReleaseVersion,
  ) -> Result<ReleaseSummary, ClientError> {
    self.move_release(device, version, "withdraw").await
  }

  /// `PATCH /admin/releases/{version}`: new notes, or the release marked critical or not.
  pub async fn change_release(
    &self,
    device: &SecretValue,
    version: &ReleaseVersion,
    changes: &ReleaseChanges,
  ) -> Result<ReleaseSummary, ClientError> {
    let request = self.json(Method::PATCH, &release_path(version), changes)?;
    changed(bearer(request, device)).await
  }

  /// `DELETE /admin/releases/{version}/files`: deletes an old release's files; the release stays in the list.
  pub async fn delete_release_files(&self, device: &SecretValue, version: &ReleaseVersion) -> Result<(), ClientError> {
    let path = format!("{}/files", release_path(version));
    send(bearer(self.request(Method::DELETE, &path)?, device)).await
  }

  async fn move_release(
    &self,
    device: &SecretValue,
    version: &ReleaseVersion,
    action: &str,
  ) -> Result<ReleaseSummary, ClientError> {
    let path = format!("{}/{action}", release_path(version));
    changed(bearer(self.request(Method::POST, &path)?, device)).await
  }
}

fn release_path(version: &ReleaseVersion) -> String {
  format!("/admin/releases/{version}")
}

/// The release as a change left it.
async fn changed(request: reqwest::RequestBuilder) -> Result<ReleaseSummary, ClientError> {
  call::<ReleaseResponse>(request).await.map(|answer| answer.release)
}
