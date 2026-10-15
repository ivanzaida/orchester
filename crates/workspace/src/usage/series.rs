//! The page's series, by the account usage started on (`orchester_core::usage::AccountOrder`): how each reads
//! (*Codex · Client*, *Other accounts · 2*, *No account*), which have usage in the range, and the Account
//! filter's options in series order, each with the series it is drawn in.

use std::sync::Arc;

use orchester_core::{
  account::AccountCli,
  id::AgentAccountId,
  usage::{AccountSeries, AccountSplit, AccountUsage},
};

use super::{
  Dashboard,
  filters::{FilterOption, FilterValue, ValueLabel},
};

/// How a series reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeriesLabel {
  /// A connected account.
  Account(AccountLabel),
  /// Other accounts, holding this many accounts with usage in the range.
  Other(usize),
  /// Other accounts holding one account with usage in the range: it reads as that account, drawn grey.
  OtherOne(AccountLabel),
  NoAccount,
}

/// How an account reads: its CLI and its name as Settings shows it (`None`: *Default*), or that it is no
/// longer connected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccountLabel {
  Known { cli: AccountCli, name: Option<Arc<str>> },
  Removed,
}

impl Dashboard {
  /// `usage` by series.
  pub fn split(&self, usage: &AccountUsage) -> AccountSplit {
    self.order.split(usage)
  }

  /// The series with usage in the range, in drawing order; a series without any is left out of the legend,
  /// the tooltip and the table, and keeps its colour.
  pub fn shown_series(&self) -> Vec<AccountSeries> {
    AccountSeries::ALL
      .into_iter()
      .filter(|series| self.range_split.of(*series) != Default::default())
      .collect()
  }

  /// How `series` reads on this page.
  pub fn series_label(&self, series: AccountSeries) -> SeriesLabel {
    match series {
      AccountSeries::Account(_) => self.order.account(series).map_or(SeriesLabel::Other(0), |account| {
        SeriesLabel::Account(self.account_label(account))
      }),
      AccountSeries::Other => match self.range_split.others.as_slice() {
        [(account, _)] => SeriesLabel::OtherOne(self.account_label(*account)),
        others => SeriesLabel::Other(others.len()),
      },
      AccountSeries::NoAccount => SeriesLabel::NoAccount,
    }
  }

  /// How `account` reads.
  pub fn account_label(&self, account: AgentAccountId) -> AccountLabel {
    self
      .names
      .account(account)
      .map_or(AccountLabel::Removed, |known| AccountLabel::Known {
        cli: known.cli(),
        name: known.name().map(|name| name.as_str().into()),
      })
  }

  /// `account`'s place among the connected accounts in series order; one no longer connected, or none,
  /// after them.
  pub fn account_place(&self, account: Option<AgentAccountId>) -> usize {
    account
      .and_then(|account| self.order.accounts().position(|connected| connected == account))
      .unwrap_or(usize::MAX)
  }

  /// The series `account`'s usage is drawn in: its colour on the page.
  pub fn account_series(&self, account: Option<AgentAccountId>) -> AccountSeries {
    self.order.series(account)
  }

  /// The Account filter's options: every connected account in series order, by CLI, then *No account*.
  pub(super) fn account_options(&self) -> Vec<FilterOption> {
    let filter = &self.query.filter;
    let accounts = self.order.accounts().map(|account| FilterOption {
      value: FilterValue::Account(account),
      label: account_value_label(self.account_label(account), account),
      chosen: filter.accounts.contains(&account),
      runs: None,
      series: Some(self.order.series(Some(account))),
    });
    let none = FilterOption {
      value: FilterValue::NoAccount,
      label: ValueLabel::NoAccount,
      chosen: filter.no_account,
      runs: None,
      series: Some(AccountSeries::NoAccount),
    };
    accounts.chain([none]).collect()
  }
}

/// An account's label as a filter value reads.
pub(super) fn account_value_label(label: AccountLabel, account: AgentAccountId) -> ValueLabel {
  match label {
    AccountLabel::Known { cli, name } => ValueLabel::Account { cli, name },
    AccountLabel::Removed => ValueLabel::Unknown(account.to_string().into()),
  }
}

impl SeriesLabel {
  /// The CLI the series' account belongs to, for a label that puts it first.
  pub const fn cli(&self) -> Option<AccountCli> {
    match self {
      Self::Account(AccountLabel::Known { cli, .. }) | Self::OtherOne(AccountLabel::Known { cli, .. }) => Some(*cli),
      Self::Account(AccountLabel::Removed)
      | Self::OtherOne(AccountLabel::Removed)
      | Self::Other(_)
      | Self::NoAccount => None,
    }
  }
}
