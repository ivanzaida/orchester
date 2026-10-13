//! What Settings › Accounts does with the accounts, off the UI thread: checks who is signed in, adds, renames and
//! removes an account, and opens a sign-in terminal and waits for it to close, then checks again. Orchester never sees
//! what is typed in that terminal.

use std::path::PathBuf;

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::{account::MoveTo, id::AgentAccountId, name::Name};
use orchester_workspace::{
  accounts::{NewAccount, NewFolder},
  background,
  messages::Message,
  workspace::Workspace,
};

use crate::services::Services;

/// A change to the accounts a person asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Change {
  Add(NewAccount),
  Rename(AgentAccountId, Name),
  Remove {
    account: AgentAccountId,
    move_to: MoveTo,
    trash: bool,
  },
  /// Keeps the account in a folder the person chose after its own went missing.
  ChooseFolder(AgentAccountId, PathBuf),
}

/// What a change did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Changed {
  /// An account was added, in a folder Orchester created (`true`) or the person's own.
  Added(AgentAccountId, bool),
  Renamed(AgentAccountId),
  Removed,
  FolderChosen(AgentAccountId),
}

impl DevtoolsInspectable for Changed {}

pub(super) type ChangeAction = FutureAction<Change, Changed, Message>;
pub(super) type CheckAction = FutureAction<Vec<AgentAccountId>, (), Message>;
pub(super) type SignInAction = FutureAction<AgentAccountId, (), Message>;
/// The folder the system's picker gave for an account, if it gave one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ChosenFolder(pub Option<(AgentAccountId, PathBuf)>);
impl DevtoolsInspectable for ChosenFolder {}
pub(super) type ChooseAction = FutureAction<AgentAccountId, ChosenFolder, Message>;

/// Asks the system's folder picker where an account's folder is now.
pub(super) fn choose_folder(ctx: &mut Ctx) -> ChooseAction {
  ctx.future_action(|account: AgentAccountId| async move {
    background::run(move || ChosenFolder(rfd::FileDialog::new().pick_folder().map(|folder| (account, folder)))).await
  })
}

/// Adds, renames or removes an account.
pub(super) fn change(ctx: &mut Ctx) -> ChangeAction {
  let workspace = Services::of(ctx).workspace;
  ctx.future_action(move |change: Change| {
    let workspace = workspace.clone();
    async move { background::run(move || apply(&workspace, change)).await? }
  })
}

fn apply(workspace: &Workspace, change: Change) -> Result<Changed, Message> {
  let failed = |error: orchester_workspace::accounts::AccountsError| {
    tracing::warn!(%error, "an account change was not made");
    error.message()
  };
  match change {
    Change::Add(new) => {
      let created = new.folder == NewFolder::Create;
      let account = workspace.add_agent_account(new).map_err(failed)?;
      if !created {
        // A brought folder is checked at once; the account stands even if the check could not run.
        if let Err(error) = workspace.check_agent_account(account.id()) {
          tracing::warn!(%error, "a brought account folder could not be checked");
        }
      }
      Ok(Changed::Added(account.id(), created))
    }
    Change::Rename(id, name) => workspace
      .rename_agent_account(id, name)
      .map(|account| Changed::Renamed(account.id()))
      .map_err(failed),
    Change::Remove {
      account,
      move_to,
      trash,
    } => workspace
      .remove_agent_account(account, move_to, trash)
      .map(|()| Changed::Removed)
      .map_err(failed),
    Change::ChooseFolder(account, folder) => workspace
      .choose_agent_account_folder(account, &folder)
      .map(|account| Changed::FolderChosen(account.id()))
      .map_err(failed),
  }
}

/// Checks the accounts given, all at once.
pub(super) fn check(ctx: &mut Ctx) -> CheckAction {
  let workspace = Services::of(ctx).workspace;
  ctx.future_action(move |accounts: Vec<AgentAccountId>| {
    let workspace = workspace.clone();
    async move {
      // Each check runs on the blocking pool at once; the results are taken in order.
      let checks: Vec<_> = accounts
        .into_iter()
        .map(|account| {
          let workspace = workspace.clone();
          tokio::task::spawn_blocking(move || workspace.check_agent_account(account))
        })
        .collect();
      for check in checks {
        match check.await {
          Ok(Ok(_)) => {}
          Ok(Err(error)) => tracing::warn!(%error, "an account could not be checked"),
          Err(error) => tracing::error!(%error, "an account check did not finish"),
        }
      }
      Ok(())
    }
  })
}

/// Opens a terminal running the account's CLI login, waits until its command ends and checks the account again.
pub(super) fn sign_in(ctx: &mut Ctx) -> SignInAction {
  let workspace = Services::of(ctx).workspace;
  ctx.future_action(move |account: AgentAccountId| {
    let workspace = workspace.clone();
    async move {
      background::run(move || {
        let terminal = workspace.open_sign_in(account).map_err(|error| error.message())?;
        if let Err(error) = terminal.wait() {
          tracing::warn!(%error, "the sign-in terminal could not be waited for; checking now");
        }
        workspace
          .check_agent_account(account)
          .map(drop)
          .map_err(|error| error.message())
      })
      .await?
    }
  })
}
