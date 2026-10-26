//! Where a pipeline's page is, kept in its route: its tab (Steps, Settings or Versions, `IsU5u`, `A29iv`,
//! `josCI`) and, on Steps, what the column beside the flow shows: the pipeline's summary, a step
//! (`/project/<id>/pipelines/<id>?step=review`), a step's Permissions, Sources or Instructions
//! (`?step=review&page=permissions`), the Deliver step or the publish review (`?panel=publish`); of the draft
//! or of a published version viewed from Versions (`?version=1`). `?panel=history`, the history panel's
//! link before the tabs, still opens Versions.

use std::num::NonZeroU32;

use orchester_core::resource::{StepKey, VersionNumber};

use super::{
  RouteError,
  query::{Params, with_query},
};

const STEP: &str = "step";
const PAGE: &str = "page";
const PANEL: &str = "panel";
const VERSION: &str = "version";
/// Versions' name before it was a tab, which links made then still use.
const HISTORY: &str = "history";

/// A tab of a pipeline's page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PipelineTab {
  Steps,
  Settings,
  Versions,
}

/// A page of a step's column, opened from its row (`T46Jq`, `z4g9jL`, `PexNQ`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StepPage {
  Permissions,
  Sources,
  Instructions,
}

impl StepPage {
  const ALL: [Self; 3] = [Self::Permissions, Self::Sources, Self::Instructions];

  pub const fn name(self) -> &'static str {
    match self {
      Self::Permissions => "permissions",
      Self::Sources => "sources",
      Self::Instructions => "instructions",
    }
  }
}

/// What a pipeline's page shows; the Steps tab with nothing chosen is where it opens.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum PipelineView {
  /// The Steps tab, its column showing the pipeline's summary.
  #[default]
  Steps,
  /// The Steps tab with a step chosen, by its key; the summary when the draft has no such step.
  Step(StepKey),
  /// A page of a step's column.
  StepPage(StepKey, StepPage),
  /// The Deliver step chosen (`?panel=deliver`), which offers to add the step when the pipeline has none.
  Deliver,
  /// The publish review in the column.
  Publish,
  Settings,
  Versions,
}

impl lurq::app::component::DevtoolsInspectable for PipelineView {}

impl PipelineView {
  const NAMED: [Self; 4] = [Self::Deliver, Self::Publish, Self::Settings, Self::Versions];

  const fn name(&self) -> Option<&'static str> {
    match self {
      Self::Deliver => Some("deliver"),
      Self::Publish => Some("publish"),
      Self::Settings => Some("settings"),
      Self::Versions => Some("versions"),
      Self::Steps | Self::Step(_) | Self::StepPage(..) => None,
    }
  }

  /// The tab this view is on.
  pub const fn tab(&self) -> PipelineTab {
    match self {
      Self::Settings => PipelineTab::Settings,
      Self::Versions => PipelineTab::Versions,
      Self::Steps | Self::Step(_) | Self::StepPage(..) | Self::Deliver | Self::Publish => PipelineTab::Steps,
    }
  }

  /// The step whose column shows, on its own fields or one of its pages.
  pub const fn step(&self) -> Option<&StepKey> {
    match self {
      Self::Step(key) | Self::StepPage(key, _) => Some(key),
      _ => None,
    }
  }

  /// The path of this view of `pipeline`, of the draft or, with `version`, of that published version.
  pub(super) fn path(&self, pipeline: String, version: Option<VersionNumber>) -> String {
    let version = version.map(|v| v.get().to_string());
    let version = (VERSION, version.as_deref());
    match self {
      Self::Steps => with_query(pipeline, &[version]),
      Self::Step(key) => with_query(pipeline, &[version, (STEP, Some(key.as_str()))]),
      Self::StepPage(key, page) => with_query(
        pipeline,
        &[version, (STEP, Some(key.as_str())), (PAGE, Some(page.name()))],
      ),
      view => with_query(pipeline, &[version, (PANEL, view.name())]),
    }
  }

  /// The published version a pipeline's path views, if any.
  pub(super) fn take_version(params: &mut Params) -> Result<Option<VersionNumber>, RouteError> {
    Ok(params.take_parsed::<NonZeroU32>(VERSION)?.map(VersionNumber::new))
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    let step = params.take(STEP)?;
    let page = params.take(PAGE)?;
    let panel = params.take(PANEL)?;
    match (step, page, panel) {
      (None, None, None) => Ok(Self::Steps),
      (Some(step), page, None) => {
        let key = StepKey::new(&step).map_err(|_| RouteError::InvalidParam {
          param: STEP.to_owned(),
          value: step,
        })?;
        match page {
          None => Ok(Self::Step(key)),
          Some(page) => StepPage::ALL
            .into_iter()
            .find(|named| named.name() == page)
            .map(|named| Self::StepPage(key, named))
            .ok_or(RouteError::InvalidParam {
              param: PAGE.to_owned(),
              value: page,
            }),
        }
      }
      (None, None, Some(panel)) if panel == HISTORY => Ok(Self::Versions),
      (None, None, Some(panel)) => Self::NAMED
        .into_iter()
        .find(|named| named.name() == Some(panel.as_str()))
        .ok_or_else(|| RouteError::InvalidParam {
          param: PANEL.to_owned(),
          value: panel,
        }),
      // A page names a step's page; without a step, or with another panel, it is no place the page shows.
      (None, Some(page), _) => Err(RouteError::InvalidParam {
        param: PAGE.to_owned(),
        value: page,
      }),
      (Some(_), _, Some(panel)) => Err(RouteError::InvalidParam {
        param: PANEL.to_owned(),
        value: panel,
      }),
    }
  }
}
