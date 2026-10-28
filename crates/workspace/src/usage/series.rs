//! The page's series, by the agent provider usage ran on (`orchester_core::usage::ProviderOrder`): how each reads
//! (*Claude · DeepSeek*, *Other providers · 2*, *Earlier runs*), which have usage in the range, and the Provider
//! filter's options in series order, each with the series it is drawn in. *Plan limits* draws CLI accounts, whose
//! limits they are: how an account reads and the colour of its lines.

use std::sync::Arc;

use orchester_core::{
  account::AccountCli,
  id::{AgentAccountId, ConnectionId},
  run::TokenUsage,
  usage::{AccountSeries, ProviderSeries, ProviderSplit, ProviderUsage, SpentOn},
};

use super::{
  Dashboard,
  filters::{FilterOption, FilterValue, ValueLabel},
};

/// A series with usage in the range: how it reads, and its usage over the range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownSeries {
  pub series: ProviderSeries,
  pub label: SeriesLabel,
  pub usage: TokenUsage,
}

/// How a series reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeriesLabel {
  /// A connected provider.
  Provider(ProviderLabel),
  /// Other providers, holding this many providers with usage in the range.
  Other(usize),
  /// Other providers holding one provider with usage in the range: it reads as that provider, drawn grey.
  OtherOne(ProviderLabel),
  /// Usage no record names a provider for.
  Earlier,
}

/// How an agent provider reads: its name as the Connections page shows it; or, once it is gone, the name it had when
/// it was last used (`None` when no record kept one).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderLabel {
  Connected { name: Arc<str> },
  Removed { name: Option<Arc<str>> },
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
  pub fn split(&self, usage: &ProviderUsage) -> ProviderSplit {
    self.order.split(usage)
  }

  /// The series with usage in the range, in drawing order; a series without any is left out of the legend,
  /// the tooltip and the table, and keeps its colour.
  pub fn shown_series(&self) -> Vec<ShownSeries> {
    ProviderSeries::ALL
      .into_iter()
      .filter_map(|series| {
        let usage = self.range_split.of(series);
        if usage == TokenUsage::default() {
          return None;
        }
        let label = match series {
          // Usage in a provider's own series ran on the connected provider at its place.
          ProviderSeries::Provider(_) => SeriesLabel::Provider(self.provider_label(self.order.provider(series)?)),
          ProviderSeries::Other => match self.range_split.others.as_slice() {
            [(connection, _)] => SeriesLabel::OtherOne(self.provider_label(*connection)),
            others => SeriesLabel::Other(others.len()),
          },
          ProviderSeries::Earlier => SeriesLabel::Earlier,
        };
        Some(ShownSeries { series, label, usage })
      })
      .collect()
  }

  /// The providers Other holds in `split` (a bar's), each on a row of its own under Other while Other reads as
  /// *Other providers · N*, so a bar holding one of them still names it; none while Other reads as its one
  /// provider.
  pub fn opened_others<'a>(&self, split: &'a ProviderSplit) -> &'a [(ConnectionId, TokenUsage)] {
    if self.range_split.others.len() == 1 {
      &[]
    } else {
      &split.others
    }
  }

  /// How the provider of `connection` reads.
  pub fn provider_label(&self, connection: ConnectionId) -> ProviderLabel {
    match self.names.provider(connection) {
      Some(provider) if !provider.removed => ProviderLabel::Connected {
        name: provider.name.clone(),
      },
      Some(provider) => ProviderLabel::Removed {
        name: Some(provider.name.clone()),
      },
      None => ProviderLabel::Removed { name: None },
    }
  }

  /// The Provider filter's options: every connected provider in series order, then *Earlier runs*.
  pub(super) fn provider_options(&self) -> Vec<FilterOption> {
    let filter = &self.query.filter;
    let providers = self.order.providers().map(|connection| FilterOption {
      value: FilterValue::Provider(connection),
      label: ValueLabel::Provider(self.provider_label(connection)),
      chosen: filter.providers.contains(&connection),
      runs: None,
      series: Some(self.order.series(SpentOn::Provider(connection))),
    });
    let earlier = FilterOption {
      value: FilterValue::Earlier,
      label: ValueLabel::Earlier,
      chosen: filter.earlier,
      runs: None,
      series: Some(ProviderSeries::Earlier),
    };
    providers.chain([earlier]).collect()
  }

  /// How `account`, whose plan limits a line draws, reads.
  pub fn account_label(&self, account: AgentAccountId) -> AccountLabel {
    self
      .names
      .account(account)
      .map_or(AccountLabel::Removed, |known| AccountLabel::Known {
        cli: known.cli(),
        name: known.name().map(|name| name.as_str().into()),
      })
  }

  /// `account`'s place among the connected accounts in Settings' order; one no longer connected, or none, after
  /// them.
  pub fn account_place(&self, account: Option<AgentAccountId>) -> usize {
    self.accounts.place(account)
  }

  /// The colour of `account`'s plan limit lines.
  pub fn account_series(&self, account: Option<AgentAccountId>) -> AccountSeries {
    self.accounts.series(account)
  }
}
