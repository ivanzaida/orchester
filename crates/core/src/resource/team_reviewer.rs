//! The team's versioned reviewer. Connection references carry no URL, key or computer-local id.

mod destinations;

pub use destinations::{ReviewerDestinations, ReviewerHost, ReviewerHostError};
use serde::{Deserialize, Serialize};

use super::{ModelId, ResourceBody, ResourceKind};
use crate::{connection::AgentProvider, name::Name, run::review::Probability};

/// A language connection by the same kind, name and role shared profiles use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LanguageReviewerReference {
  Connection { name: Name, role: AgentProvider },
}

/// A decision connection is a separate kind, never usable as an agent connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DecisionReviewerReference {
  DecisionConnection { name: Name, role: DecisionReviewerRole },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionReviewerRole {
  Decision,
}

/// Language reviewers carry effort; decision reviewers carry a minimum threshold and permitted destinations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TeamReviewer {
  Language {
    connection: LanguageReviewerReference,
    model: ModelId,
    effort: Option<String>,
  },
  Decision {
    connection: DecisionReviewerReference,
    model: ModelId,
    lowest_threshold: Probability,
    destinations: ReviewerDestinations,
  },
}

impl TeamReviewer {
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
  /// Language reviewers never widen; the first version is compared with members' computers only.
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
