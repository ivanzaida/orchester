//! Servers per owner (the person's decisions, 2026-10-16 and 2026-10-30): the person and automation reach only this
//! computer's servers and the signed-in account's, and another account's server is not found
//! ([`SharingError::ServerMissing`]), whatever asks for it: a screen, a domain tool or the Orchestrator chat. The
//! background work that keeps every server's connection and every shared project in step goes on for each owner's
//! servers while another is signed in, and shows the signed-in account nothing of it: each shared project syncs through
//! its own owner's server only. So a project and its server always have the same owner: a first link makes it so once
//! the server accepted it ([`crate::workspace::Workspace::link_with_owner`]), and a shared project does not move
//! ([`Servers::move_project`]).

use std::path::Path;

use orchester_core::id::{CloudAccountId, ServerId};
use orchester_store::{ProjectDb, RegisteredProject};

use super::{ServerEntry, Servers, SharingError};
use crate::{
  background,
  workspace::{MoveError, MoveTo},
};

/// On whose behalf a request reaches a server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Behalf {
  /// The signed-in account, for the person or automation: only this computer's servers and its own.
  Account,
  /// The background work that keeps each owner's servers and shared projects in step, and shows nothing.
  Background,
}

impl Servers {
  /// Every server in the list, whichever account's, with its connection: for the background work only. Blocking.
  pub(crate) fn every_entry(&self) -> Result<Vec<ServerEntry>, SharingError> {
    self.listed(self.inner.workspace.every_server()?)
  }

  /// `owner`'s servers with their connections: for the background work on `owner`'s shared projects. Blocking.
  pub(crate) fn entries_of(&self, owner: Option<&CloudAccountId>) -> Result<Vec<ServerEntry>, SharingError> {
    self.listed(self.inner.workspace.servers_of(owner)?)
  }

  /// Refuses `server` unless the signed-in account uses it: another account's server is not found.
  pub(super) async fn own(&self, server: ServerId) -> Result<(), SharingError> {
    let workspace = self.inner.workspace.clone();
    if background::run(move || workspace.is_own_server(server)).await?? {
      Ok(())
    } else {
      Err(SharingError::ServerMissing)
    }
  }

  /// Moves `project` as a person asked on All projects. Refused like a shared project while it is being shared now: a
  /// first link read whose it is, and no share starts while this runs. Blocking.
  pub fn move_project(&self, project: &RegisteredProject, to: MoveTo) -> Result<(), MoveError> {
    self
      .outside_share(&project.repo_root, || self.inner.workspace.move_project(project, to))
      .unwrap_or(Err(MoveError::Shared))
  }

  /// Refuses, before anything is sent, linking the project at `repo_root` to `entry` when their owners cannot meet
  /// ([`crate::workspace::Workspace::check_link_owner`]).
  pub(super) async fn check_link_owner(&self, entry: &ServerEntry, repo_root: &Path) -> Result<(), SharingError> {
    let (workspace, root, server) = (self.inner.workspace.clone(), repo_root.to_owned(), entry.stored.id);
    background::run(move || -> Result<(), SharingError> {
      let project = ProjectDb::open(&root)?.project().id();
      workspace.check_link_owner(project, server)
    })
    .await?
  }
}
