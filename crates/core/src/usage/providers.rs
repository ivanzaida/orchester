//! The series usage is drawn in: the agent providers it ran on, which are the project's agent connections as Project
//! settings › Connections lists them under *Agent providers* (decided 2026-10-28, in place of the CLI accounts). One
//! series per provider, in that order (by name; across projects, project by project); the first three keep their colour
//! by that place, whatever the period or filter; the fourth and later share *Other providers*, with providers no longer
//! connected that still have usage; usage no record names a provider for (turns and reviews from before the provider
//! was recorded, whose step's records name none either) is *Earlier runs*, always last. Command tools spend no model
//! tokens: they are in no series.

use std::sync::Arc;

use crate::{id::ConnectionId, run::TokenUsage};

/// How many series there are: three providers, Other providers, Earlier runs.
pub const SERIES: usize = 5;
/// How many connected providers keep a series, and a colour, of their own.
pub const OWN_SERIES: usize = 3;

/// What usage was spent on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SpentOn {
  /// The agent connection it ran on.
  Provider(ConnectionId),
  /// No record names the connection: usage from before it was recorded.
  Earlier,
}

impl SpentOn {
  /// On `connection`, or Earlier runs when none is known.
  pub fn of(connection: Option<ConnectionId>) -> Self {
    connection.map_or(Self::Earlier, Self::Provider)
  }

  pub const fn connection(self) -> Option<ConnectionId> {
    match self {
      Self::Provider(connection) => Some(connection),
      Self::Earlier => None,
    }
  }
}

/// The agent connection an invocation or a review starts on, recorded with its name then, so that its usage still
/// names it once the connection is gone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsedProvider {
  pub id: ConnectionId,
  pub name: Arc<str>,
}

/// An agent provider as usage names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderName {
  pub id: ConnectionId,
  /// Its name as the Connections page shows it; once it is gone, the name it had when it was last used.
  pub name: Arc<str>,
  /// It is no longer among the project's connections.
  pub removed: bool,
}

/// One series, in drawing order: stacks build bottom up, legends and columns read left to right.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProviderSeries {
  /// The connected provider at this place in series order.
  Provider(ProviderPlace),
  Other,
  Earlier,
}

/// A place (0, 1 or 2) among the connected providers that keep a series of their own; only
/// [`ProviderSeries::ALL`] and [`ProviderOrder::series`] make one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProviderPlace(usize);

impl ProviderPlace {
  pub const fn get(self) -> usize {
    self.0
  }
}

impl ProviderSeries {
  pub const ALL: [Self; SERIES] = [
    Self::Provider(ProviderPlace(0)),
    Self::Provider(ProviderPlace(1)),
    Self::Provider(ProviderPlace(2)),
    Self::Other,
    Self::Earlier,
  ];

  /// Its place among [`Self::ALL`], which its colour follows.
  pub const fn index(self) -> usize {
    match self {
      Self::Provider(place) => place.0,
      Self::Other => OWN_SERIES,
      Self::Earlier => OWN_SERIES + 1,
    }
  }
}

/// The connected providers in series order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderOrder {
  providers: Vec<ConnectionId>,
}

impl ProviderOrder {
  /// The connected ones of `providers`, in the order given: as the Connections page lists them.
  pub fn of(providers: &[ProviderName]) -> Self {
    let mut order = Vec::new();
    for provider in providers.iter().filter(|provider| !provider.removed) {
      if !order.contains(&provider.id) {
        order.push(provider.id);
      }
    }
    Self { providers: order }
  }

  /// Every connected provider in series order.
  pub fn providers(&self) -> impl Iterator<Item = ConnectionId> + '_ {
    self.providers.iter().copied()
  }

  /// The series usage spent `on` counts in: a provider's own while it is one of the first three connected ones,
  /// Other for a later one or one no longer connected, Earlier runs where no provider is known.
  pub fn series(&self, on: SpentOn) -> ProviderSeries {
    let SpentOn::Provider(connection) = on else {
      return ProviderSeries::Earlier;
    };
    match self.providers.iter().position(|id| *id == connection) {
      Some(place) if place < OWN_SERIES => ProviderSeries::Provider(ProviderPlace(place)),
      _ => ProviderSeries::Other,
    }
  }

  /// The connected provider a series of its own stands for.
  pub fn provider(&self, series: ProviderSeries) -> Option<ConnectionId> {
    match series {
      ProviderSeries::Provider(place) => self.providers.get(place.0).copied(),
      ProviderSeries::Other | ProviderSeries::Earlier => None,
    }
  }

  /// `connection`'s place among the connected providers; one no longer connected after them.
  pub fn place(&self, connection: ConnectionId) -> usize {
    self
      .providers
      .iter()
      .position(|id| *id == connection)
      .unwrap_or(usize::MAX)
  }

  /// `usage` by series, with the providers in Other each on its own, in series order (providers no longer
  /// connected after the connected ones).
  pub fn split(&self, usage: &ProviderUsage) -> ProviderSplit {
    let mut split = ProviderSplit::default();
    for (on, spent) in &usage.0 {
      let series = self.series(*on);
      split.series[series.index()] += *spent;
      if let (ProviderSeries::Other, SpentOn::Provider(connection)) = (series, on) {
        split.others.push((*connection, *spent));
      }
    }
    split
      .others
      .sort_by_key(|(connection, _)| (self.place(*connection), *connection));
    split
  }
}

/// Usage by what it was spent on, each provider once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderUsage(Vec<(SpentOn, TokenUsage)>);

impl ProviderUsage {
  pub fn add(&mut self, on: SpentOn, usage: TokenUsage) {
    match self.0.iter_mut().find(|(of, _)| *of == on) {
      Some((_, sum)) => *sum += usage,
      None => self.0.push((on, usage)),
    }
  }

  /// Each provider's usage, in the order first seen.
  pub fn entries(&self) -> &[(SpentOn, TokenUsage)] {
    &self.0
  }
}

/// Usage by series, and Other's providers each on its own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderSplit {
  /// Indexed by [`ProviderSeries::index`].
  pub series: [TokenUsage; SERIES],
  /// The providers Other holds, in series order, with their usage.
  pub others: Vec<(ConnectionId, TokenUsage)>,
}

impl ProviderSplit {
  pub fn of(&self, series: ProviderSeries) -> TokenUsage {
    self.series[series.index()]
  }
}
