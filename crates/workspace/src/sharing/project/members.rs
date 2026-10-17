//! The members of a shared project and their roles (`docs/architecture/server.md`, decisions 17 and
//! 25; design 10.13, 10.14): owners and the server's admins add people by their exact username and
//! change roles, the last owner can neither step down nor leave, and anyone may leave. After the
//! person leaves, or makes themself a member, the cache records it. Since API 3 the app shows the members read-only
//! (row 63) and keeps them as last read, so they show while the server cannot be reached ([`ProjectMembers`]).

use std::path::Path;

use orchester_api::{ErrorCode, Member, MemberChanges, MemberCreate, MemberRole};
use orchester_core::{id::UserId, time::Timestamp};
use orchester_store::{ProjectDb, SharedMember, StoreError};

use super::ProjectServer;
use crate::{
  background,
  sharing::{Servers, SharingError},
};

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
  pub read: MembersRead,
  /// The person among them, as this computer was signed in when it read them: still known once it signed out.
  pub me: Option<UserId>,
}

/// Whether the members were read from the server now, and why not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MembersRead {
  /// The server listed them now.
  Live,
  /// As last read: the server cannot be reached now.
  Unreachable,
  /// As last read: this computer is signed out of the server, or the server is no longer in the list.
  NoServer,
  /// As last read: the server shows its members only to members, and the person is not one any more.
  NotMember,
}

impl ProjectMembers {
  /// The members who are the project's owners.
  pub fn owners(&self) -> impl Iterator<Item = &SharedMember> {
    self.members.iter().filter(|member| member.role == MemberRole::Owner)
  }

  pub fn live(&self) -> bool {
    self.read == MembersRead::Live
  }
}

impl Servers {
  /// The members of the project at `repo_root` as Sharing shows them ([`ProjectServer::current_members`]), also once
  /// its server is no longer in the list: then as last read.
  pub async fn project_members(&self, repo_root: &Path) -> Result<ProjectMembers, SharingError> {
    let (servers, root) = (self.clone(), repo_root.to_owned());
    let server = background::run(move || ProjectServer::open(&servers, &root)).await?;
    match server {
      Ok(Some(server)) => server.current_members().await,
      Ok(None) => Err(StoreError::NotShared.into()),
      Err(SharingError::ServerMissing) => last_read(repo_root, MembersRead::NoServer)
        .await?
        .ok_or(SharingError::ServerMissing),
      Err(error) => Err(error),
    }
  }
}

impl ProjectServer {
  /// The members as the server lists them now, kept as the cache; while it cannot be reached, once this computer is
  /// signed out of it, or once the person is no longer a member, as last read (the error when they never were).
  pub async fn current_members(&self) -> Result<ProjectMembers, SharingError> {
    let root = self.repo_root.clone();
    // The server shows a project's members only to its members: once the person is not one, the list is as last read.
    let read = match self.link.removed_at {
      Some(_) => Err(SharingError::NotMember),
      None => self.members().await,
    };
    let error = match read {
      Ok(members) => {
        let mut members: Vec<SharedMember> = members.into_iter().map(shared_member).collect();
        // Owners first, then by when they were added, as the cache keeps them.
        members.sort_by_key(|member| (member.role != MemberRole::Owner, member.added_at));
        let (kept, read_at, me) = (members.clone(), Timestamp::now(), self.me());
        background::run(move || ProjectDb::open(&root)?.record_shared_members(&kept, read_at, me)).await??;
        return Ok(ProjectMembers {
          members,
          read_at,
          read: MembersRead::Live,
          me,
        });
      }
      Err(error) => error,
    };
    let Some(why) = as_last_read(&error) else {
      return Err(error);
    };
    if why == MembersRead::NotMember && self.link.removed_at.is_none() {
      // The server answers a non-member like no project; its sync records since when the person is not one.
      self.servers.sync_now();
    }
    last_read(&root, why).await?.ok_or(error)
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

/// Why the members are shown as last read after `error`: the server could not be asked, or no longer shows them to
/// the person; `None` for an error the person is told instead.
fn as_last_read(error: &SharingError) -> Option<MembersRead> {
  match error {
    SharingError::Offline | SharingError::CertificateChanged => Some(MembersRead::Unreachable),
    SharingError::SignedOut => Some(MembersRead::NoServer),
    SharingError::NotMember => Some(MembersRead::NotMember),
    SharingError::Client(error) if error.is_connection() => Some(MembersRead::Unreachable),
    SharingError::Client(error) if matches!(error.code(), Some(ErrorCode::NotFound | ErrorCode::NotMember)) => {
      Some(MembersRead::NotMember)
    }
    _ => None,
  }
}

/// The members of the project at `root` as last read, `read` for why; `None` when they never were.
async fn last_read(root: &Path, read: MembersRead) -> Result<Option<ProjectMembers>, SharingError> {
  let root = root.to_owned();
  let cached = background::run(move || {
    let db = ProjectDb::open(&root)?;
    let me = db.members_read_as()?;
    Ok::<_, StoreError>(db.shared_members()?.map(|read| (read, me)))
  })
  .await??;
  Ok(cached.map(|((read_at, members), me)| ProjectMembers {
    members,
    read_at,
    read,
    me,
  }))
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
