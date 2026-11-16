//! Where a link to a pipeline opens: the Pipelines hub with the pipeline's panel (`Pipelines · v2`), in a local project
//! and a team project alike. A route to the page a pipeline had before the hub (a link from before, Back, a restored
//! tab, the MCP) is turned into the hub's where it is shown ([`open`], [`page_with`]).

use orchester_core::{id::ResourceId, resource::VersionNumber};

use crate::{
  routes::{PipelineView, PipelinesFilter, PipelinesView, ProjectPage},
  screens::pipeline_hub::OpenStep,
};

/// The page a link to `pipeline` opens. The link names it under *All*, as the link cannot know whether it was archived
/// since; the hub opens the tab that holds it (`pipeline_hub::model::Listing::landing`).
pub(in crate::screens) fn page(pipeline: ResourceId) -> ProjectPage {
  ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(pipeline)))
}

/// What the panel holds open for `view` of the edit, or of saved `version`: a step, one of its pages or the Deliver
/// step; a saved version, the Steps, Settings and Versions tabs and the review before a save show the pipeline's panel
/// itself, which holds them all (its flow, its settings, *Versions* in its head, *Save*).
pub(in crate::screens) fn open(view: &PipelineView, version: Option<VersionNumber>) -> Option<OpenStep> {
  match (view, version) {
    (PipelineView::Step(key) | PipelineView::StepPage(key, _), None) => Some(OpenStep::Step(key.clone())),
    (PipelineView::Deliver, None) => Some(OpenStep::Deliver),
    _ => None,
  }
}

/// The hub's own route for pipeline `pipeline` with `open` open.
pub(in crate::screens) fn page_with(pipeline: ResourceId, open: Option<&OpenStep>) -> ProjectPage {
  match open {
    Some(open) => ProjectPage::Pipeline(pipeline, open.view()),
    None => page(pipeline),
  }
}
