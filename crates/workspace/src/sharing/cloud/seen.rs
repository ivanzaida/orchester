//! The session's activity follows the account signed in (the person's decision, 2026-10-16: projects per account).
//! An account's runners keep working in the background while it is signed out, and what they report stays out of the
//! activity the status bar shows; when that account signs in again, what they report shows again.

use orchester_core::id::CloudAccountId;

use super::Cloud;
use crate::{activity::Activity, background};

/// Keeps `activity` to the projects shown now: whenever another account signs in (or none is), what the activity holds
/// of the projects it does not see goes, and nothing more of them is recorded. Runs until the session ends.
pub async fn keep_activity_to_the_account(cloud: Cloud, activity: Activity) {
  let mut sign_in = cloud.follow_sign_in();
  let mut kept_to: Option<Option<CloudAccountId>> = None;
  loop {
    let account = sign_in.borrow_and_update().account();
    if kept_to != Some(account) {
      let workspace = cloud.inner.workspace.clone();
      match background::run(move || workspace.unseen_roots()).await {
        Ok(Ok(hidden)) => {
          activity.hide_projects(hidden);
          kept_to = Some(account);
        }
        Ok(Err(error)) => tracing::error!(%error, "cannot read which projects the signed-in account sees"),
        // `background::run` logged why the work did not finish.
        Err(_) => {}
      }
    }
    if sign_in.changed().await.is_err() {
      return;
    }
  }
}
