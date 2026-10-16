//! What a pipeline's editor shows beside its graph (`vEDYY`), kept in its route: the pipeline's summary, a
//! step (`/project/<id>/pipelines/<id>?step=review`), or its settings, history or publish review
//! (`?panel=history`).

use orchester_core::resource::StepKey;

use super::{
  RouteError,
  query::{Params, with_query},
};

const STEP: &str = "step";
const PANEL: &str = "panel";

/// A panel of the pipeline editor; the pipeline's summary is the one it opens on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum PipelinePanel {
  #[default]
  Pipeline,
  /// A step's inspector, by the step's key; the summary when the draft has no such step.
  Step(StepKey),
  Settings,
  History,
  Publish,
}

impl lurq::app::component::DevtoolsInspectable for PipelinePanel {}

impl PipelinePanel {
  const NAMED: [Self; 3] = [Self::Settings, Self::History, Self::Publish];

  const fn name(&self) -> Option<&'static str> {
    match self {
      Self::Settings => Some("settings"),
      Self::History => Some("history"),
      Self::Publish => Some("publish"),
      Self::Pipeline | Self::Step(_) => None,
    }
  }

  pub(super) fn path(&self, pipeline: String) -> String {
    match self {
      Self::Pipeline => pipeline,
      Self::Step(key) => with_query(pipeline, &[(STEP, Some(key.as_str()))]),
      panel => with_query(pipeline, &[(PANEL, panel.name())]),
    }
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    let step = params.take(STEP)?;
    let panel = params.take(PANEL)?;
    match (step, panel) {
      (None, None) => Ok(Self::Pipeline),
      (Some(step), None) => StepKey::new(&step)
        .map(Self::Step)
        .map_err(|_| RouteError::InvalidParam {
          param: STEP.to_owned(),
          value: step,
        }),
      (None, Some(panel)) => Self::NAMED
        .into_iter()
        .find(|named| named.name() == Some(panel.as_str()))
        .ok_or_else(|| RouteError::InvalidParam {
          param: PANEL.to_owned(),
          value: panel,
        }),
      // A step and another panel at once is no place the editor shows.
      (Some(_), Some(panel)) => Err(RouteError::InvalidParam {
        param: PANEL.to_owned(),
        value: panel,
      }),
    }
  }
}
