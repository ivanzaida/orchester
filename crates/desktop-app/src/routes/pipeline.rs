//! What a pipeline's editor shows beside its graph (`vEDYY`), kept in its route: the pipeline's summary, a
//! step (`/project/<id>/pipelines/<id>?step=review`), or its settings, history or publish review
//! (`?panel=history`), of the draft or of a published version viewed from the history
//! (`?version=1&panel=history`).

use std::num::NonZeroU32;

use orchester_core::resource::{StepKey, VersionNumber};

use super::{
  RouteError,
  query::{Params, with_query},
};

const STEP: &str = "step";
const PANEL: &str = "panel";
const VERSION: &str = "version";

/// A panel of the pipeline editor; the pipeline's summary is the one it opens on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum PipelinePanel {
  #[default]
  Pipeline,
  /// A step's inspector, by the step's key; the summary when the draft has no such step.
  Step(StepKey),
  /// The Deliver step's inspector (`?panel=deliver`), which adds the step when the pipeline has none.
  Deliver,
  Settings,
  History,
  Publish,
}

impl lurq::app::component::DevtoolsInspectable for PipelinePanel {}

impl PipelinePanel {
  const NAMED: [Self; 4] = [Self::Deliver, Self::Settings, Self::History, Self::Publish];

  const fn name(&self) -> Option<&'static str> {
    match self {
      Self::Deliver => Some("deliver"),
      Self::Settings => Some("settings"),
      Self::History => Some("history"),
      Self::Publish => Some("publish"),
      Self::Pipeline | Self::Step(_) => None,
    }
  }

  /// The path of this panel of `pipeline`, of the draft or, with `version`, of that published version.
  pub(super) fn path(&self, pipeline: String, version: Option<VersionNumber>) -> String {
    let version = version.map(|v| v.get().to_string());
    let version = (VERSION, version.as_deref());
    match self {
      Self::Pipeline => with_query(pipeline, &[version]),
      Self::Step(key) => with_query(pipeline, &[version, (STEP, Some(key.as_str()))]),
      panel => with_query(pipeline, &[version, (PANEL, panel.name())]),
    }
  }

  /// The published version a pipeline's path views, if any.
  pub(super) fn take_version(params: &mut Params) -> Result<Option<VersionNumber>, RouteError> {
    Ok(params.take_parsed::<NonZeroU32>(VERSION)?.map(VersionNumber::new))
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
