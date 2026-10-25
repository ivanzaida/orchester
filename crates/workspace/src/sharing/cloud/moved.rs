//! The public cloud moved to its own domain (2026-10-25): [`FORMER_PUBLIC_ADDRESS`] and [`PUBLIC_ADDRESS`] are one
//! service with one database, so a device token signed in at the former address works at the new one. What this
//! computer stored with the former address moves there at the start, and the sign-in stays: the account's address
//! and the advanced setting, when it names the public cloud by hand. Any other cloud stays as it is.

use orchester_api::client::ServerAddress;
use orchester_store::{AppDb, StoreError};

use super::{BUILD_ADDRESS, FORMER_PUBLIC_ADDRESS, PUBLIC_ADDRESS};
use crate::workspace::Workspace;

/// Moves this computer's stored cloud addresses off the former public address. Blocking. A database that refuses
/// keeps them: the former address still answers, and the next start tries again.
pub(super) fn move_from_former_address(workspace: &Workspace) {
  if let Err(error) = move_stored(&mut workspace.db.lock()) {
    tracing::warn!(%error, "this computer keeps the public cloud's former address for now");
  }
}

fn move_stored(db: &mut AppDb) -> Result<(), StoreError> {
  if db.cloud_address()?.is_some_and(|text| is_former(&text)) {
    // Named by hand, the public cloud is the build's own again, unless this build uses another cloud.
    let address = (BUILD_ADDRESS != PUBLIC_ADDRESS).then_some(PUBLIC_ADDRESS);
    db.set_cloud_address(address)?;
  }
  if let Some(mut account) = db.cloud_account()?
    && is_former(&account.cloud_address)
  {
    account.cloud_address = PUBLIC_ADDRESS.to_owned();
    db.set_cloud_account(Some(&account))?;
  }
  Ok(())
}

/// Whether `text` is the former public address, as the person may have typed it.
fn is_former(text: &str) -> bool {
  ServerAddress::parse(text).is_ok_and(|address| address.as_str() == FORMER_PUBLIC_ADDRESS)
}
