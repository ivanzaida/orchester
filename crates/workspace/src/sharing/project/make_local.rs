//! *Make local* (`docs/architecture/server.md`, "Migration"; design 10.14 G, H): the person's
//! claims are released first when the server can be reached, and otherwise go stale on their own;
//! then every cached task becomes a local task of the project with its id, number and organization
//! as last synced. The membership stays: joining again later works. The server project is unchanged
//! for everyone else. The tasks kept *Not shared* when the project joined return to the list with
//! numbers after the highest (10.26 C); they are returned with their keys before and after.

use orchester_store::{ProjectDb, ReturnedTask};

use super::ProjectServer;
use crate::{background, sharing::SharingError};

impl ProjectServer {
  pub async fn make_local(&self) -> Result<Vec<ReturnedTask>, SharingError> {
    if self.writable().is_ok()
      && let Some(me) = self.me()
    {
      let root = self.repo_root.clone();
      let held = background::run(move || ProjectDb::open(&root)?.claims_held_by(me)).await??;
      for task in held {
        let released = self
          .call(|client, token, project| async move { client.release_claim(&token, project, task).await })
          .await;
        if let Err(error) = released {
          // The claim goes stale on its own after 15 minutes, as when the server cannot be reached.
          tracing::warn!(%error, %task, "a claim was not released on Make local");
        }
      }
    }
    let root = self.repo_root.clone();
    let returned = background::run(move || ProjectDb::open(&root)?.make_local()).await??;
    self.servers.shared_changed();
    Ok(returned)
  }
}
