//! The team's versioned reviewer (`docs/design/team-configuration.md`, "Who owns what"). A language reviewer names the
//! CLI kind it runs with, like a team agent profile, and a decision reviewer names no connection at all: its kind
//! (`decision`) implies a decision connection. Each computer picks the connection it runs on
//! (`team_connection_picks`), so the body holds no name, URL, key or computer-local id, and its JSON refuses any other
//! field: a body that names a connection (the form before ORC-887) is not a team reviewer and is not read.

mod destinations;

pub use destinations::{ReviewerDestinations, ReviewerHost, ReviewerHostError};
use serde::{Deserialize, Serialize};

use super::{ModelId, ResourceBody, ResourceKind};
use crate::{connection::AgentProvider, run::review::Probability};

/// Language reviewers carry a CLI kind and effort; decision reviewers carry a minimum threshold and permitted
/// destinations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TeamReviewer {
  Language {
    /// The agent provider a connection must have to run it; each computer picks one of its own of that kind.
    cli: AgentProvider,
    model: ModelId,
    effort: Option<String>,
  },
  Decision {
    model: ModelId,
    lowest_threshold: Probability,
    destinations: ReviewerDestinations,
  },
}

impl TeamReviewer {
  /// The CLI kind a language reviewer runs with; `None` for a decision reviewer, which needs a decision connection.
  pub const fn cli(&self) -> Option<AgentProvider> {
    match self {
      Self::Language { cli, .. } => Some(*cli),
      Self::Decision { .. } => None,
    }
  }

  /// A computer may require more confidence than the team, never less. Language reviewers have no threshold.
  pub fn effective_threshold(&self, member: Option<Probability>) -> Option<Probability> {
    let Self::Decision { lowest_threshold, .. } = self else {
      return None;
    };
    Some(
      member
        .filter(|member| member > lowest_threshold)
        .unwrap_or(*lowest_threshold),
    )
  }

  /// Only decision reviewers send reviewed requests to a decision server.
  pub fn allows_server(&self, server: &crate::connection::DecisionServerUrl) -> bool {
    match self {
      Self::Language { .. } => false,
      Self::Decision { destinations, .. } => destinations.allows_server(server),
    }
  }

  /// A version widens if it adds destinations or lowers the team's decision threshold.
  /// Language reviewers never widen, whatever CLI kind they name (a profile's CLI kind does not widen either); the
  /// first version is compared with members' computers only.
  pub fn widens_over(&self, previous: Option<&Self>) -> bool {
    !crate::sharing::SharedBody::widenings_over(self, previous).is_empty()
  }
}

impl ResourceBody for TeamReviewer {
  const KIND: ResourceKind = ResourceKind::TeamReviewer;
  const SCHEMA: u32 = 1;
}

impl crate::sharing::SharedBody for TeamReviewer {
  fn widenings_over(&self, previous: Option<&Self>) -> Vec<crate::sharing::VersionWidening> {
    use crate::sharing::VersionWidening;

    let Self::Decision {
      destinations,
      lowest_threshold,
      ..
    } = self
    else {
      return Vec::new();
    };
    let (before, threshold) = match previous {
      Some(Self::Decision {
        destinations,
        lowest_threshold,
        ..
      }) => (destinations, Some(lowest_threshold)),
      _ => (&ReviewerDestinations::MembersComputers, None),
    };
    let mut adds = Vec::new();
    if destinations.adds_to(before) {
      adds.push(VersionWidening::Address);
    }
    if threshold.is_some_and(|before| lowest_threshold < before) {
      adds.push(VersionWidening::Access);
    }
    adds
  }
}
