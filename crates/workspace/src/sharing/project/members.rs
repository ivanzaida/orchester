//! The members of a shared project and their roles (`docs/architecture/server.md`, decisions 17 and
//! 25; design 10.13, 10.14): owners and the server's admins add people by their exact username and
//! change roles, the last owner can neither step down nor leave, and anyone may leave. After the
//! person leaves, or makes themself a member, the cache records it. Since API 3 the app shows the members read-only
//! (row 63) and keeps them as last read, so they show while the server cannot be reached ([`ProjectMembers`]).

use orchester_api::{Member, MemberChanges, MemberCreate, MemberRole};
use orchester_core::{id::UserId, time::Timestamp};
use orchester_store::{ProjectDb, SharedMember};

use super::ProjectServer;
use crate::{background, sharing::SharingError};

/// A change of one member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberChange {
  Role(UserId, MemberRole),
  /// Removes the member; the person's own row is *Leave*.
  Remove(UserId),
}

/// The members of a shared project as Sharing shows them: as the server listed them now, or as last read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectMembers {
  pub members: Vec<SharedMember>,
  pub read_at: Timestamp,
  /// Read from the server now; otherwise as last read, while it cannot be reached.
  pub live: bool,
}

impl ProjectMembers {
  /// The members who are the project's owners.
  pub fn owners(&self) -> impl Iterator<Item = &SharedMember> {
    self.members.iter().filter(|member| member.role == MemberRole::Owner)
  }
}

impl ProjectServer {
  /// The members as the server lists them now, kept as the cache; while it cannot be reached, or once the person is
  /// no longer a member, as last read (the error when they never were).
  pub async fn current_members(&self) -> Result<ProjectMembers, SharingError> {
    let root = self.repo_root.clone();
    // The server shows a project's members only to its members: once the person is not one, the list is as last read.
    let read = match self.link.removed_at {
      Some(_) => Err(SharingError::NotMember),
      None => self.members().await,
    };
    match read {
      Ok(members) => {
        let members: Vec<SharedMember> = members.into_iter().map(shared_member).collect();
        let (kept, read_at) = (members.clone(), Timestamp::now());
        background::run(move || ProjectDb::open(&root)?.record_shared_members(&kept, read_at)).await??;
        Ok(ProjectMembers {
          members,
          read_at,
          live: true,
        })
      }
      Err(error) if unreachable(&error) => {
        let cached = background::run(move || ProjectDb::open(&root)?.shared_members()).await??;
        let (read_at, members) = cached.ok_or(error)?;
        Ok(ProjectMembers {
          members,
          read_at,
          live: false,
        })
      }
      Err(error) => Err(error),
    }
  }

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

/// Whether `error` says the server could not be asked, or no longer shows the members to the person, so they are
/// shown as last read.
fn unreachable(error: &SharingError) -> bool {
  match error {
    SharingError::Offline | SharingError::CertificateChanged | SharingError::NotMember => true,
    SharingError::Client(error) => error.is_connection(),
    _ => false,
  }
}

fn shared_member(member: Member) -> SharedMember {
  SharedMember {
    user: member.user.id,
    display_name: member.user.display_name,
    username: member.username,
    role: member.role,
    added_at: Timestamp::from_unix_millis(member.added_at),
  }
}

/// Members are read while the server can be reached, also by a person who is no longer one: the
/// server then says so.
fn reachable_member(server: &ProjectServer) -> Result<(), SharingError> {
  super::reachable(server.servers.connection(server.link.server))
}
