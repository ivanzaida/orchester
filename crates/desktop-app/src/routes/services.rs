//! The pages under Project settings › Services (`Agent testing · redesign` `eWydp`): a service's, a stack's and a test
//! suite's editor, their New pages, Import draft and Flaky tests
//! (`/project/<id>/settings?tab=services&view=service&id=…`, `view=new-service`, `view=import`, `view=flaky`, …). Until
//! their tasks land, each is a page that says it is not built yet.

use orchester_core::id::ResourceId;

use super::{
  ProjectSettingsTab, RouteError,
  query::{Params, with_query},
};

/// The query parameter of the service, stack or suite whose page shows.
const ID: &str = "id";

/// A page under the Services tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ServicesPage {
  NewService,
  NewStack,
  NewSuite,
  /// Import draft: a first draft from the repository's compose files, `package.json` scripts and Cargo binaries.
  Import,
  /// Flaky tests and their quarantine.
  Flaky,
  Service(ResourceId),
  Stack(ResourceId),
  Suite(ResourceId),
}

impl ServicesPage {
  /// The pages named without an id, as `view` names them.
  pub const NAMED: [Self; 5] = [
    Self::NewService,
    Self::NewStack,
    Self::NewSuite,
    Self::Import,
    Self::Flaky,
  ];

  /// The page's name in a path's `view` and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::NewService => "new-service",
      Self::NewStack => "new-stack",
      Self::NewSuite => "new-suite",
      Self::Import => "import",
      Self::Flaky => "flaky",
      Self::Service(_) => "service",
      Self::Stack(_) => "stack",
      Self::Suite(_) => "suite",
    }
  }

  /// The item the page opens, if it opens one.
  pub const fn item(self) -> Option<ResourceId> {
    match self {
      Self::Service(id) | Self::Stack(id) | Self::Suite(id) => Some(id),
      _ => None,
    }
  }

  /// The page `view` names without an id.
  pub fn named(view: &str) -> Option<Self> {
    Self::NAMED.into_iter().find(|page| page.name() == view)
  }

  /// The page's path under Project settings at `settings`.
  pub(super) fn path(self, settings: String) -> String {
    let id = self.item().map(|id| id.to_string());
    with_query(
      settings,
      &[
        ProjectSettingsTab::Services.param(),
        (super::page::VIEW, Some(self.name())),
        (ID, id.as_deref()),
      ],
    )
  }

  /// The page `view` names, with the id the query holds for an item's.
  pub(super) fn take(view: String, params: &mut Params) -> Result<Self, RouteError> {
    if let Some(page) = Self::named(&view) {
      return Ok(page);
    }
    let item: fn(ResourceId) -> Self = match view.as_str() {
      "service" => Self::Service,
      "stack" => Self::Stack,
      "suite" => Self::Suite,
      _ => {
        return Err(RouteError::InvalidParam {
          param: super::page::VIEW.to_owned(),
          value: view,
        });
      }
    };
    params.take_parsed(ID)?.map(item).ok_or(RouteError::NoScreen)
  }
}
