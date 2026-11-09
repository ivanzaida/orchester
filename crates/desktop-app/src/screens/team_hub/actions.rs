//! What the panels' fixes write, the profile's and the MCP server's: each is one action of the workspace's fill-ins,
//! run off the UI thread. The pick, the program path and switching a server off go on as the workspace takes them;
//! turning a server on and a secret's value are a person's alone, and the window checks that before it runs the action.

use std::path::Path;

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::{
  id::{ConnectionId, ResourceId},
  run::Answerer,
  secret::{SecretName, SecretValue},
};
use orchester_store::ConnectionPick;
use orchester_workspace::{background, messages::Message};

use crate::services::{
  Services,
  path_picker::{PathKind, PathPicker},
};

/// Whether a fix can be pressed now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::screens) enum Busy {
  Idle,
  /// A fix is being taken.
  Taking,
  /// The hub is read again, and what the panel shows is its last answer.
  Reading,
}

#[derive(Clone)]
pub(super) struct Actions {
  /// Keeps the connection (second) as the profile's (first) on this computer.
  pub pick: FutureAction<(ResourceId, ConnectionId), (), Message>,
  /// Asks for the program of the MCP server, then sets it.
  pub set_path: FutureAction<ResourceId, (), Message>,
  pub switch_on: FutureAction<ResourceId, (), Message>,
  /// Switching off is always taken, also for automation; only a person's press reaches it here.
  pub switch_off: FutureAction<ResourceId, (), Message>,
  /// Keeps the value in the keychain under the secret's name. It holds the value as a [`SecretValue`] from the dialog
  /// on, which shows nothing of it.
  pub add_value: FutureAction<(SecretName, SecretValue), (), Message>,
}

impl Actions {
  pub(super) fn new(ctx: &mut Ctx, root: &Path) -> Self {
    let services = Services::of(ctx);
    let (workspace, vault) = (services.workspace, services.vault);
    let root = root.to_path_buf();
    let picker = PathPicker::of(ctx);
    let title = ctx.t("team_hub.fix.programTitle");
    let pick = {
      let (workspace, root) = (workspace.clone(), root.clone());
      ctx.future_action(move |(profile, connection): (ResourceId, ConnectionId)| {
        let (workspace, root) = (workspace.clone(), root.clone());
        async move {
          let pick = ConnectionPick {
            connection,
            account: None,
          };
          background::run(move || workspace.pick_team_connection(&root, profile, pick))
            .await?
            .map_err(|error| error.message())
        }
      })
    };
    let set_path = {
      let (workspace, root) = (workspace.clone(), root.clone());
      ctx.future_action(move |server: ResourceId| {
        let (workspace, root, picker, title) = (workspace.clone(), root.clone(), picker.clone(), title.clone());
        async move {
          let Some(program) = picker.pick(PathKind::File, title).await else {
            return Ok(());
          };
          background::run(move || workspace.set_team_mcp_program(&root, server, &program, Answerer::Person))
            .await?
            .map_err(|error| error.message())
        }
      })
    };
    let switch_on = {
      let (workspace, root) = (workspace.clone(), root.clone());
      ctx.future_action(move |server: ResourceId| {
        let (workspace, root) = (workspace.clone(), root.clone());
        async move {
          background::run(move || workspace.switch_team_mcp_on_here(&root, server, true, Answerer::Person))
            .await?
            .map_err(|error| error.message())
        }
      })
    };
    let switch_off = {
      let (workspace, root) = (workspace.clone(), root.clone());
      ctx.future_action(move |server: ResourceId| {
        let (workspace, root) = (workspace.clone(), root.clone());
        async move {
          background::run(move || workspace.switch_team_mcp_on_here(&root, server, false, Answerer::Person))
            .await?
            .map_err(|error| error.message())
        }
      })
    };
    let add_value = ctx.future_action(move |(name, value): (SecretName, SecretValue)| {
      let (workspace, root, vault) = (workspace.clone(), root.clone(), vault.clone());
      async move {
        background::run(move || workspace.set_team_secret_value(&root, vault.get(), &name, &value, Answerer::Person))
          .await?
          .map_err(|error| error.message())
      }
    });
    Self {
      pick,
      set_path,
      switch_on,
      switch_off,
      add_value,
    }
  }

  /// Whether a fix can be pressed: not while the hub is read again, whose last answer it would act on, nor while one is
  /// being taken.
  pub(super) fn busy(&self, reading: bool) -> Busy {
    if reading {
      Busy::Reading
    } else if self.pending() {
      Busy::Taking
    } else {
      Busy::Idle
    }
  }

  /// Whether a fix of the connection, the path or the server is being taken.
  pub(super) fn pending(&self) -> bool {
    self.pick.state().get().is_pending()
      || self.set_path.state().get().is_pending()
      || self.switch_on.state().get().is_pending()
      || self.switch_off.state().get().is_pending()
  }

  /// Why the last fix of the connection, the path or the server did nothing, as the workspace said.
  pub(super) fn refused(&self) -> Option<Message> {
    [
      self.pick.state().get(),
      self.set_path.state().get(),
      self.switch_on.state().get(),
      self.switch_off.state().get(),
    ]
    .into_iter()
    .find_map(|state| state.error.clone().filter(|_| !state.is_pending()))
  }
}
