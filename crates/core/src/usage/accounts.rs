//! The colours *Plan limits* draws its lines in: the CLI accounts whose limits they are (plan limits belong to the
//! accounts, decided 2026-09-30), while token usage is split by agent provider ([`super::ProviderOrder`]). One colour
//! per connected account, in the order Settings lists them by CLI (Codex, then Claude Code; in each, the named accounts
//! as listed, Default last); the first three keep their colour by that position, the fourth and later, and accounts
//! removed from Settings, share *Other*'s.

use super::providers::OWN_SERIES;
use crate::{
  account::{AccountCli, AgentAccount},
  id::AgentAccountId,
};

/// The colour of an account's plan limit lines, in the palette the provider series use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountSeries {
  /// The connected account at this place (0, 1 or 2) in Settings' order.
  Account(usize),
  Other,
}

impl AccountSeries {
  /// Its place in the palette: the provider series' at the same place ([`super::ProviderSeries::index`]).
  pub const fn index(self) -> usize {
    match self {
      Self::Account(place) => place,
      Self::Other => OWN_SERIES,
    }
  }
}

/// The connected accounts in Settings' order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountOrder {
  accounts: Vec<AgentAccountId>,
}

impl AccountOrder {
  /// `accounts`, the connected ones as Settings keeps them: by CLI, Codex first; in each CLI the named accounts in the
  /// order they were added, Default last.
  pub fn of(accounts: &[AgentAccount]) -> Self {
    let mut sorted: Vec<&AgentAccount> = accounts.iter().collect();
    sorted.sort_by_key(|account| {
      (
        cli_rank(account.cli()),
        account.is_default(),
        account.added_at(),
        account.id(),
      )
    });
    Self {
      accounts: sorted.into_iter().map(AgentAccount::id).collect(),
    }
  }

  /// `account`'s place among the connected accounts; one no longer connected, or none, after them.
  pub fn place(&self, account: Option<AgentAccountId>) -> usize {
    account
      .and_then(|account| self.accounts.iter().position(|id| *id == account))
      .unwrap_or(usize::MAX)
  }

  /// The colour of `account`'s lines: its own while it is one of the first three connected accounts, else Other's.
  pub fn series(&self, account: Option<AgentAccountId>) -> AccountSeries {
    match self.place(account) {
      place if place < OWN_SERIES => AccountSeries::Account(place),
      _ => AccountSeries::Other,
    }
  }
}

/// Codex's accounts come first, then Claude Code's, as the page orders CLIs.
const fn cli_rank(cli: AccountCli) -> u8 {
  match cli {
    AccountCli::Codex => 0,
    AccountCli::ClaudeCode => 1,
  }
}
