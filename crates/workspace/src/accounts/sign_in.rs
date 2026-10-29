//! Signing an account in with its CLI's own login (decision 2): Orchester opens a terminal window the person sees,
//! running the login with the account's folder variable set, and shows the same command to run in a terminal of
//! one's own. Orchester never sees what is typed, never passes a key or token and never reads the credential file;
//! when the window's command ends, the caller checks the account again.

use std::{env, path::PathBuf};

use orchester_core::{
  account::{AgentAccount, Shell, sign_in_command},
  connection::{
    ConnectionAccount,
    invocation::{Platform, agent_environment},
  },
  id::AgentAccountId,
};
use orchester_exec::sign_in_terminal::{Login, SignInTerminal};
use orchester_store::StoreError;

use super::AccountsError;
use crate::{connections::probe, workspace::Workspace};

/// The login to run in a terminal of one's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignInCommand {
  /// The shell it is written for (*PowerShell*, *zsh*), a name, not translated.
  pub shell: &'static str,
  pub text: String,
}

/// The login of `account` for this platform's shell.
pub fn command(account: &AgentAccount) -> SignInCommand {
  let shell = Shell::of(Platform::current());
  let (variable, name) = match shell {
    Shell::PowerShell => ("LOCALAPPDATA", "$env:LOCALAPPDATA"),
    Shell::Zsh => ("HOME", "$HOME"),
  };
  let base = env::var_os(variable).map(PathBuf::from);
  SignInCommand {
    shell: shell.name(),
    text: sign_in_command(
      account.cli(),
      account.folder().path(),
      shell,
      base.as_deref().map(|base| (base, name)),
    ),
  }
}

impl Workspace {
  /// Opens a terminal window running the login of account `id`'s CLI for its folder.
  pub fn open_sign_in(&self, id: AgentAccountId) -> Result<SignInTerminal, AccountsError> {
    let account = self.agent_account(id)?.ok_or(StoreError::AgentAccountNotFound(id))?;
    let cli = account.cli();
    if let Some(folder) = account.folder().path()
      && !folder.is_dir()
    {
      return Err(AccountsError::FolderMissing(folder.to_path_buf()));
    }
    let program = probe::detect(cli.program())
      .into_iter()
      .next()
      .ok_or(AccountsError::CliMissing)?;
    let connection = ConnectionAccount::of(&account);
    let scratch = self
      .account_root
      .as_deref()
      .and_then(|root| root.parent())
      .map_or_else(env::temp_dir, |data| data.join("sign-in"));
    let login = Login {
      program: &program,
      args: cli.login_args(),
      variable: connection.variable(cli.provider()),
      environment: &agent_environment(env::vars_os()),
      scratch: &scratch,
    };
    SignInTerminal::open(&login).map_err(AccountsError::Terminal)
  }
}
