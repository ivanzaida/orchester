//! Where a link from another page to a pipeline opens: in a local project the Pipelines hub with the pipeline's panel
//! (`Pipelines · v2`), in a team project the pipeline's own page ([`PipelineLinks`]). The choice is made where the link
//! is built; a route to the old page that a local project is still asked for (a link from before, Back, a restored
//! tab, the MCP) is turned into the hub's where it is shown ([`LocalPipeline`]).

use orchester_core::{id::ResourceId, resource::VersionNumber};

use crate::{
  project::{ShownProject, TeamRead},
  routes::{PipelineView, PipelinesFilter, PipelinesView, ProjectPage},
  screens::pipeline_hub::OpenStep,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::screens) enum PipelineLinks {
  /// The project is local: a link opens the hub's panel.
  Hub,
  /// The project is shared, or not yet known to be local: a link opens the pipeline's page, as it did before the hub.
  /// The link is built again once the read says the project is local.
  Page,
}

impl PipelineLinks {
  pub(in crate::screens) fn of(project: &ShownProject) -> Self {
    project.feed.team.with(|team| match team {
      TeamRead::Read(None) => Self::Hub,
      TeamRead::Read(Some(_)) | TeamRead::Unread | TeamRead::Failed(_) => Self::Page,
    })
  }

  /// The page a link to `pipeline` opens. The link names it under *All*, as the link cannot know whether it was
  /// archived since; the hub opens the tab that holds it (`pipeline_hub::model::Listing::landing`).
  pub(in crate::screens) fn page(self, pipeline: ResourceId) -> ProjectPage {
    match self {
      Self::Hub => ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(pipeline))),
      Self::Page => ProjectPage::Pipeline(pipeline, PipelineView::default()),
    }
  }
}

/// A route to a pipeline's own page, as a local project shows it: always the Pipelines hub with the pipeline's panel.
pub(in crate::screens) struct LocalPipeline;

impl LocalPipeline {
  /// What the panel holds open for `view` of the edit, or of published `version`: a step, one of its pages or the
  /// Deliver step; a published version, the Steps, Settings and Versions tabs and the publish review show the
  /// pipeline's panel itself, which holds them all (its flow, its settings, *Versions* in its head, *Save*).
  pub(in crate::screens) fn open(view: &PipelineView, version: Option<VersionNumber>) -> Option<OpenStep> {
    match (view, version) {
      (PipelineView::Step(key) | PipelineView::StepPage(key, _), None) => Some(OpenStep::Step(key.clone())),
      (PipelineView::Deliver, None) => Some(OpenStep::Deliver),
      _ => None,
    }
  }

  /// The hub's own route for pipeline `pipeline` with `open` open.
  pub(in crate::screens) fn page(pipeline: ResourceId, open: Option<&OpenStep>) -> ProjectPage {
    match open {
      Some(open) => ProjectPage::Pipeline(pipeline, open.view()),
      None => PipelineLinks::Hub.page(pipeline),
    }
  }
}
