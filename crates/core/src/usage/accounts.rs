//! The series usage is drawn in: the CLI accounts it started on (`docs/design/orchester-workspace.md`, "Usage
//! dashboard", series rules in `P0pTa` of `data/orchester2.pen`). One series per connected account, in the
//! order Settings lists them by CLI (Codex, then Claude Code; in each, the named accounts as listed, Default
//! last); the first three keep their colour by that position, whatever the period, filter or project; the
//! fourth and later share *Other accounts*, with accounts removed from Settings that still have usage; usage
//! that started on no account (auto mode's reviewer, native agents, command tools, runs from before accounts
//! were recorded) is *No account*, always last.

use crate::{
  account::{AccountCli, AgentAccount},
  id::AgentAccountId,
  run::TokenUsage,
};

/// How many series there are: three accounts, Other accounts, No account.
pub const SERIES: usize = 5;
/// How many connected accounts keep a series, and a colour, of their own.
pub const OWN_SERIES: usize = 3;

/// One series, in drawing order: stacks build bottom up, legends and columns read left to right.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountSeries {
  /// The connected account at this place (0, 1 or 2) in series order.
  Account(usize),
  Other,
  NoAccount,
}

impl AccountSeries {
  pub const ALL: [Self; SERIES] = [
    Self::Account(0),
    Self::Account(1),
    Self::Account(2),
    Self::Other,
    Self::NoAccount,
  ];

  /// Its place among [`Self::ALL`], which its colour follows.
  pub const fn index(self) -> usize {
    match self {
      Self::Account(place) => place,
      Self::Other => OWN_SERIES,
      Self::NoAccount => OWN_SERIES + 1,
    }
  }
}

/// The connected accounts in series order, with their CLI.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountOrder {
  accounts: Vec<(AgentAccountId, AccountCli)>,
}

impl AccountOrder {
  /// `accounts`, the connected ones as Settings keeps them, in series order: by CLI, Codex first; in each
  /// CLI the named accounts in the order they were added, Default last.
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
      accounts: sorted
        .into_iter()
        .map(|account| (account.id(), account.cli()))
        .collect(),
    }
  }

  /// Every connected account in series order.
  pub fn accounts(&self) -> impl Iterator<Item = AgentAccountId> + '_ {
    self.accounts.iter().map(|(id, _)| *id)
  }

  /// The connected accounts of `cli`, in series order.
  pub fn of_cli(&self, cli: AccountCli) -> impl Iterator<Item = AgentAccountId> + '_ {
    self
      .accounts
      .iter()
      .filter(move |(_, of)| *of == cli)
      .map(|(id, _)| *id)
  }

  /// The CLIs with connected accounts, in series order.
  pub fn clis(&self) -> Vec<AccountCli> {
    let mut clis: Vec<AccountCli> = self.accounts.iter().map(|(_, cli)| *cli).collect();
    clis.dedup();
    clis
  }

  /// The series usage that started on `account` counts in: its own while it is one of the first three
  /// connected accounts, Other for a later one or one no longer connected, No account for none.
  pub fn series(&self, account: Option<AgentAccountId>) -> AccountSeries {
    let Some(account) = account else {
      return AccountSeries::NoAccount;
    };
    match self.accounts.iter().position(|(id, _)| *id == account) {
      Some(place) if place < OWN_SERIES => AccountSeries::Account(place),
      _ => AccountSeries::Other,
    }
  }

  /// The connected account a series of its own stands for.
  pub fn account(&self, series: AccountSeries) -> Option<AgentAccountId> {
    match series {
      AccountSeries::Account(place) => self.accounts.get(place).map(|(id, _)| *id),
      AccountSeries::Other | AccountSeries::NoAccount => None,
    }
  }

  /// `usage` by series, with the accounts in Other each on its own, in series order (accounts no longer
  /// connected after the connected ones).
  pub fn split(&self, usage: &AccountUsage) -> AccountSplit {
    let mut split = AccountSplit::default();
    for (account, spent) in &usage.0 {
      let series = self.series(*account);
      split.series[series.index()] += *spent;
      if let (AccountSeries::Other, Some(account)) = (series, account) {
        split.others.push((*account, *spent));
      }
    }
    let place = |account: AgentAccountId| {
      self
        .accounts
        .iter()
        .position(|(id, _)| *id == account)
        .unwrap_or(usize::MAX)
    };
    split.others.sort_by_key(|(account, _)| (place(*account), *account));
    split
  }
}

/// Codex's accounts come first, then Claude Code's, as the page orders CLIs.
const fn cli_rank(cli: AccountCli) -> u8 {
  match cli {
    AccountCli::Codex => 0,
    AccountCli::ClaudeCode => 1,
  }
}

/// Usage by the account it started on (`None`: no account), each account once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountUsage(Vec<(Option<AgentAccountId>, TokenUsage)>);

impl AccountUsage {
  pub fn add(&mut self, account: Option<AgentAccountId>, usage: TokenUsage) {
    match self.0.iter_mut().find(|(of, _)| *of == account) {
      Some((_, sum)) => *sum += usage,
      None => self.0.push((account, usage)),
    }
  }

  /// Each account's usage, in the order first seen.
  pub fn entries(&self) -> &[(Option<AgentAccountId>, TokenUsage)] {
    &self.0
  }

  pub fn total(&self) -> TokenUsage {
    self.0.iter().map(|(_, usage)| *usage).sum()
  }
}

/// Usage by series, and Other's accounts each on its own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountSplit {
  /// Indexed by [`AccountSeries::index`].
  pub series: [TokenUsage; SERIES],
  /// The accounts Other holds, in series order, with their usage.
  pub others: Vec<(AgentAccountId, TokenUsage)>,
}

impl AccountSplit {
  pub fn of(&self, series: AccountSeries) -> TokenUsage {
    self.series[series.index()]
  }
}
