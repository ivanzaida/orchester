//! This app's own updates (`orchester_workspace::app_updates`): the release keys built into it, which every downloaded
//! update's signature must verify with (`docs/cloud-api.md`, "Key ids and rotation"), and this program as its updates
//! see it.

use std::path::Path;

use orchester_api::releases::{AcceptedReleaseKeys, ReleasePublicKey, ReleaseVersion};
use orchester_workspace::{
  app_updates::{AppUpdates, ThisApp},
  workspace::Workspace,
};

use crate::mcp::DISCOVERY_VARIABLE;

/// The Ed25519 public keys of the release keys this app accepts, as 64 lowercase hex characters: the current one and,
/// while one replaces another, the one before. The private key lives only in CI's secret store. Until the first release
/// key is made and listed here, every downloaded update is refused as signed with an unknown key and deleted.
const RELEASE_KEYS: &[&str] = &[];

/// The release keys built into the app.
pub fn release_keys() -> AcceptedReleaseKeys {
  AcceptedReleaseKeys::new(
    RELEASE_KEYS
      .iter()
      .map(|key| ReleasePublicKey::parse(key).expect("a built-in release key is 64 lowercase hex characters"))
      .collect(),
  )
}

/// This program: its own release version, and whether it is a QA copy (started with [`DISCOVERY_VARIABLE`]).
pub fn this_app() -> ThisApp {
  let version = ReleaseVersion::parse(env!("CARGO_PKG_VERSION")).expect("the app's version is a release version");
  let qa = std::env::var_os(DISCOVERY_VARIABLE).is_some_and(|folder| !folder.is_empty());
  ThisApp::detect(version, qa)
}

/// The updates of this program, whose settings are in `workspace` and whose downloads go to `data_folder`.
pub fn start(workspace: Workspace, data_folder: &Path) -> AppUpdates {
  AppUpdates::new(workspace, this_app(), release_keys(), data_folder)
}
