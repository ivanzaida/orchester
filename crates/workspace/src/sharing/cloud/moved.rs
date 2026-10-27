//! The public cloud moved twice: to its own domain (2026-10-25), from [`FORMER_PUBLIC_ADDRESS`] to
//! [`PUBLIC_ADDRESS`], and its API to a host of its own, [`PUBLIC_API_ADDRESS`] (`docs/cloud-api.md`, "Base"). They are
//! one service with one database and one issuer, [`PUBLIC_ADDRESS`], so a device token signed in at an earlier address
//! works at the new ones, and the identity tokens and key sets the cloud gives keep naming the same issuer. What this
//! computer stored with an earlier address of the public cloud moves to its web and API addresses at the start, and the
//! sign-in stays: the account's addresses and the advanced setting, when it names the public cloud by hand. Any other
//! cloud stays as it is.

use orchester_api::client::ServerAddress;
use orchester_store::{AppDb, StoreError, StoredCloudAddress};

use super::{BUILD_ADDRESS, BUILD_API_ADDRESS, FORMER_PUBLIC_ADDRESS, PUBLIC_ADDRESS, PUBLIC_API_ADDRESS};
use crate::workspace::Workspace;

/// Moves this computer's stored cloud addresses off the public cloud's earlier ones. Blocking. A database that refuses
/// keeps them: the earlier addresses still answer while people update, and the next start tries again.
pub(super) fn move_from_former_address(workspace: &Workspace) {
  if let Err(error) = move_stored(&mut workspace.db.lock()) {
    tracing::warn!(%error, "this computer keeps the public cloud's earlier address for now");
  }
}

fn move_stored(db: &mut AppDb) -> Result<(), StoreError> {
  if db
    .cloud_address()?
    .is_some_and(|set| set.api.is_none() && is_public(&set.web))
  {
    // Named by hand, the public cloud is the build's own again, unless this build uses another cloud.
    let build_is_public = BUILD_ADDRESS == PUBLIC_ADDRESS && BUILD_API_ADDRESS == Some(PUBLIC_API_ADDRESS);
    let address = (!build_is_public).then(|| StoredCloudAddress {
      web: PUBLIC_ADDRESS.to_owned(),
      api: Some(PUBLIC_API_ADDRESS.to_owned()),
    });
    db.set_cloud_address(address.as_ref())?;
  }
  if let Some(mut account) = db.cloud_account()?
    && account.cloud_api_address.is_none()
    && is_public(&account.cloud_address)
  {
    account.cloud_address = PUBLIC_ADDRESS.to_owned();
    account.cloud_api_address = Some(PUBLIC_API_ADDRESS.to_owned());
    db.set_cloud_account(Some(&account))?;
  }
  Ok(())
}

/// Whether `text` is the public cloud's web address, now or before 2026-10-25, as the person may have typed it.
fn is_public(text: &str) -> bool {
  ServerAddress::parse(text).is_ok_and(|address| [PUBLIC_ADDRESS, FORMER_PUBLIC_ADDRESS].contains(&address.as_str()))
}
