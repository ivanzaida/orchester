//! What a card that cannot run offers besides opening its step: the profile or tool it names, opened in the Agents hub
//! where it is published.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::id::ResourceId;
use orchester_workspace::pipelines::flow::{Runs, StepCard};

use crate::routes::{HubFilter, HubView, ProjectPage};

/// The resource a not-ready card sends the person to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::screens::pipeline_hub) enum Fix {
  Profile(ResourceId),
  Tool(ResourceId),
}

impl Fix {
  /// The resource `runs` names that is not ready, if it names one.
  pub(in crate::screens::pipeline_hub) const fn of(runs: &Runs) -> Option<Self> {
    match runs {
      Runs::ProfileNotPublished { id } => Some(Self::Profile(*id)),
      Runs::ToolNotPublished { id } | Runs::ToolNotRunnable { id, .. } => Some(Self::Tool(*id)),
      Runs::Profile { .. } | Runs::Tool { .. } | Runs::NoProfile | Runs::NoTool => None,
    }
  }

  /// What `card` offers to open: the resource it names that is not ready, unless that resource is gone (archived or
  /// removed), which has no page to open; the card then offers to choose another ([`super::words::choose`]).
  pub(super) const fn of_card(card: &StepCard) -> Option<Self> {
    if card.gone {
      return None;
    }
    Self::of(&card.runs)
  }

  /// The Agents hub on the resource.
  pub(in crate::screens::pipeline_hub) fn page(self) -> ProjectPage {
    ProjectPage::AgentsHub(match self {
      Self::Profile(id) => HubView::new(HubFilter::Profiles, Some(id)),
      Self::Tool(id) => HubView::new(HubFilter::Tools, Some(id)),
    })
  }

  /// *Open profile*, *Open tool*.
  pub(super) fn words(self, ctx: &Ctx) -> Arc<str> {
    ctx.t(match self {
      Self::Profile(_) => "pipeline_hub.flow.openProfile",
      Self::Tool(_) => "pipeline_hub.flow.openTool",
    })
  }
}
