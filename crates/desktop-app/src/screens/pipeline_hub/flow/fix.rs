//! What a card that cannot run offers besides opening its step: the profile or tool it names, opened in the Agents hub
//! where it is published.

use lurq::app::ctx::Ctx;
use orchester_core::id::ResourceId;
use orchester_workspace::pipelines::flow::Runs;
use std::sync::Arc;

use crate::routes::{HubFilter, HubView, ProjectPage};

/// The resource a not-ready card sends the person to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Fix {
  Profile(ResourceId),
  Tool(ResourceId),
}

impl Fix {
  /// The resource `runs` names that is not ready, if it names one.
  pub(super) const fn of(runs: &Runs) -> Option<Self> {
    match runs {
      Runs::ProfileNotPublished { id } => Some(Self::Profile(*id)),
      Runs::ToolNotPublished { id } | Runs::ToolNotRunnable { id, .. } => Some(Self::Tool(*id)),
      Runs::Profile { .. } | Runs::Tool { .. } | Runs::NoProfile | Runs::NoTool => None,
    }
  }

  /// The Agents hub on the resource.
  pub(super) fn page(self) -> ProjectPage {
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
