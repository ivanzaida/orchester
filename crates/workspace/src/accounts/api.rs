//! The person's API accounts (`docs/design/agent-accounts.md`, "Agents run on accounts"), app-wide in the
//! application database with each one's key in the OS keychain: listing, adding with a key, renaming, changing the
//! model API or its models, replacing the key, and the program of each agent CLI. Everything here blocks on SQLite
//! or the keychain; the UI calls it off the UI thread.

use std::path::{Path, PathBuf};

use orchester_core::{
  account::{AccountCli, ApiAccount, ApiSetup},
  id::ApiAccountId,
  name::Name,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  time::Timestamp,
};
use orchester_store::StoreError;

use super::AccountsError;
use crate::workspace::Workspace;

impl Workspace {
  /// The API accounts, by kind of agent, in the order they were added.
  pub fn api_accounts(&self) -> Result<Vec<ApiAccount>, StoreError> {
    self.db.lock().api_accounts()
  }

  /// The API account, removed or not: a run still names what it ran on.
  pub fn api_account(&self, id: ApiAccountId) -> Result<Option<ApiAccount>, StoreError> {
    self.db.lock().api_account(id)
  }

  /// Adds an API account with its key: the key goes into the keychain first, then the account into the database.
  /// When the database refuses the account (a name taken), the key is removed again.
  pub fn add_api_account(
    &self,
    vault: &dyn SecretVault,
    name: Name,
    setup: ApiSetup,
    key: &SecretValue,
  ) -> Result<ApiAccount, AccountsError> {
    self.name_free(&name, &setup, None)?;
    let account = ApiAccount::new(ApiAccountId::generate(), name, setup, Timestamp::now());
    let entry = VaultKey::api_account(account.id());
    vault.store(&entry, key).map_err(AccountsError::Keychain)?;
    if let Err(error) = self.db.lock().add_api_account(&account) {
      // The key was stored for this account only; nothing names it, so one that stays is never read.
      if let Err(left) = vault.delete(&entry) {
        tracing::warn!(%left, account = %account.id(), "the key of an API account that was not added stays in the keychain");
      }
      return Err(error.into());
    }
    self.accounts_changed();
    Ok(account)
  }

  /// Renames the API account; everything on it shows the new name, as it holds the account's id.
  pub fn rename_api_account(&self, id: ApiAccountId, name: Name) -> Result<ApiAccount, AccountsError> {
    let mut account = self.listed_api_account(id)?;
    self.name_free(&name, account.setup(), Some(id))?;
    account.rename(name);
    self.db.lock().save_api_account(&account)?;
    self.accounts_changed();
    Ok(account)
  }

  /// Points the API account at `setup`: another base URL, other models, provider fallback. The kind of agent stays.
  pub fn change_api_account(&self, id: ApiAccountId, setup: ApiSetup) -> Result<ApiAccount, AccountsError> {
    let mut account = self.listed_api_account(id)?;
    account.set_setup(setup).map_err(AccountsError::ApiAccount)?;
    self.db.lock().save_api_account(&account)?;
    self.accounts_changed();
    Ok(account)
  }

  /// Replaces the API account's key. The keychain keeps one key per account, so the previous one is gone once the
  /// new one is stored, and the check made with it is dropped.
  pub fn replace_api_account_key(
    &self,
    vault: &dyn SecretVault,
    id: ApiAccountId,
    key: &SecretValue,
  ) -> Result<ApiAccount, AccountsError> {
    let mut account = self.listed_api_account(id)?;
    vault
      .store(&VaultKey::api_account(id), key)
      .map_err(AccountsError::Keychain)?;
    account.key_replaced();
    self.db.lock().save_api_account(&account)?;
    self.accounts_changed();
    Ok(account)
  }

  /// Whether the keychain of this computer holds the API account's key, without reading it.
  pub fn api_account_has_key(&self, vault: &dyn SecretVault, id: ApiAccountId) -> Result<bool, VaultError> {
    vault.exists(&VaultKey::api_account(id))
  }

  /// The program set for `cli` on this computer, if the person set one; the file may be gone.
  pub fn agent_program(&self, cli: AccountCli) -> Result<Option<PathBuf>, StoreError> {
    self.db.lock().agent_program(cli)
  }

  /// Keeps `executable`, an absolute path, as the program of `cli` for every project.
  pub fn set_agent_program(&self, cli: AccountCli, executable: &Path) -> Result<(), AccountsError> {
    if !executable.is_absolute() {
      return Err(AccountsError::RelativeProgram(executable.to_path_buf()));
    }
    self.db.lock().set_agent_program(cli, executable, Timestamp::now())?;
    self.accounts_changed();
    Ok(())
  }

  /// Forgets the program set for `cli`, which is looked up on the `PATH` again.
  pub fn clear_agent_program(&self, cli: AccountCli) -> Result<(), StoreError> {
    if self.db.lock().clear_agent_program(cli)? {
      self.accounts_changed();
    }
    Ok(())
  }

  /// Whether an API account of `cli` goes by `name`, which a sign-in of the CLI then may not take
  /// ([`Self::name_free`]).
  pub(super) fn api_account_named(&self, cli: AccountCli, name: &Name) -> Result<bool, StoreError> {
    Ok(
      self.api_accounts()?.iter().any(|account| {
        account.agent() == cli.provider() && account.name().as_str().eq_ignore_ascii_case(name.as_str())
      }),
    )
  }

  /// The API account `id` while it is in the list.
  fn listed_api_account(&self, id: ApiAccountId) -> Result<ApiAccount, AccountsError> {
    self
      .api_accounts()?
      .into_iter()
      .find(|account| account.id() == id)
      .ok_or_else(|| StoreError::ApiAccountNotFound(id).into())
  }

  /// Refused when a sign-in of the same CLI goes by `name`: both are offered to a profile side by side (*Claude Code
  /// · Work*), where the same name twice would not say which is which. `renamed` is the API account being renamed.
  /// A name another API account of the kind of agent has is refused by the database.
  fn name_free(&self, name: &Name, setup: &ApiSetup, renamed: Option<ApiAccountId>) -> Result<(), AccountsError> {
    let taken = |other: &Name| other.as_str().eq_ignore_ascii_case(name.as_str());
    let sign_ins = self.agent_accounts()?;
    let by_sign_in = AccountCli::of(setup.agent()).is_some_and(|cli| {
      sign_ins
        .iter()
        .any(|account| account.cli() == cli && account.name().is_some_and(taken))
    });
    let by_api_account = self
      .api_accounts()?
      .iter()
      .any(|other| Some(other.id()) != renamed && other.agent() == setup.agent() && taken(other.name()));
    if by_sign_in || by_api_account {
      return Err(StoreError::DuplicateAccountName(name.as_str().to_owned()).into());
    }
    Ok(())
  }
}
