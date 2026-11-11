//! Where a link from another page to a pipeline opens: in a local project the Pipelines hub with the pipeline's panel
//! (`Pipelines · v2`), in a team project the pipeline's own page. The choice is made where the link is built, not by
//! redirecting the pipeline's route: in a local project the hub's Deliver card and *Edit steps* still open that page.

use orchester_core::id::ResourceId;

use crate::{
  project::{ShownProject, TeamRead},
  routes::{PipelineView, PipelinesFilter, PipelinesView, ProjectPage},
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

  /// The page a link to `pipeline` opens.
  pub(in crate::screens) fn page(self, pipeline: ResourceId) -> ProjectPage {
    match self {
      Self::Hub => ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(pipeline))),
      Self::Page => ProjectPage::Pipeline(pipeline, PipelineView::default()),
    }
  }
}
