//! The members of a shared project and their roles (`docs/architecture/server.md`, decisions 17 and
//! 25; design 10.13, 10.14): owners and the server's admins add people by their exact username and
//! change roles, the last owner can neither step down nor leave, and anyone may leave. After the
//! person leaves, or makes themself a member, the cache records it.

use orchester_api::{Member, MemberChanges, MemberCreate, MemberRole};
use orchester_core::{id::UserId, time::Timestamp};
use orchester_store::ProjectDb;

use super::ProjectServer;
use crate::{background, sharing::SharingError};

/// A change of one member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberChange {
  Role(UserId, MemberRole),
  /// Removes the member; the person's own row is *Leave*.
  Remove(UserId),
}

impl ProjectServer {
  /// The project's members, as the server lists them.
  pub async fn members(&self) -> Result<Vec<Member>, SharingError> {
    reachable_member(self)?;
    self
      .call(|client, token, project| async move { client.members(&token, project).await })
      .await
  }

  /// Adds the person with the exact `username` as `role`.
  pub async fn add_member(&self, username: String, role: MemberRole) -> Result<Member, SharingError> {
    self.writable()?;
    let create = MemberCreate { username, role };
    self
      .call(|client, token, project| async move { client.add_member(&token, project, &create).await })
      .await
  }

  pub async fn change_member(&self, change: MemberChange) -> Result<(), SharingError> {
    self.writable()?;
    match change {
      MemberChange::Role(user, role) => {
        let changes = MemberChanges { role };
        self
          .call(|client, token, project| async move { client.change_member(&token, project, user, &changes).await })
          .await?;
        if Some(user) == self.me() {
          self.record(Some(role), None).await?;
        }
      }
      MemberChange::Remove(user) => {
        self
          .call(|client, token, project| async move { client.remove_member(&token, project, user).await })
          .await?;
        if Some(user) == self.me() {
          self.record(None, Some(Timestamp::now())).await?;
        }
      }
    }
    self.servers.shared_changed();
    Ok(())
  }

  /// Records the person's role and whether they left, in the cache.
  async fn record(&self, role: Option<MemberRole>, removed_at: Option<Timestamp>) -> Result<(), SharingError> {
    let root = self.repo_root.clone();
    let role = role.or(self.link.role);
    background::run(move || ProjectDb::open(&root)?.record_sharing_state(role, None, removed_at)).await??;
    Ok(())
  }
}

/// Members are read while the server can be reached, also by a person who is no longer one: the
/// server then says so.
fn reachable_member(server: &ProjectServer) -> Result<(), SharingError> {
  super::reachable(server.servers.connection(server.link.server))
}
