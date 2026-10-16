//! What Instructions shows (`fePhH`): its scope tab and the rule open in it, a new rule, or the import page
//! (`/project/<id>/instructions?tab=roles&rule=…`, `&rule=new`, `?view=import`).

use orchester_core::id::ResourceId;

use super::{
  RouteError, TAB,
  query::{Params, with_query},
};

const RULE: &str = "rule";
const VIEW: &str = "view";
const NEW: &str = "new";
const IMPORT: &str = "import";

/// A scope tab of Instructions; Project rules is the one it opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InstructionsTab {
  #[default]
  Project,
  Roles,
  Pipelines,
  Steps,
}

impl InstructionsTab {
  pub const ALL: [Self; 4] = [Self::Project, Self::Roles, Self::Pipelines, Self::Steps];

  pub const fn name(self) -> &'static str {
    match self {
      Self::Project => "project",
      Self::Roles => "roles",
      Self::Pipelines => "pipelines",
      Self::Steps => "steps",
    }
  }

  fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }
}

/// A rule of a tab (`None`: the tab's first), a new rule in a tab, or the import page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InstructionsView {
  Rule(InstructionsTab, Option<ResourceId>),
  New(InstructionsTab),
  Import,
}

// The view a person left Instructions for, held while unsaved edits are asked about.
impl lurq::app::component::DevtoolsInspectable for InstructionsView {}

impl Default for InstructionsView {
  fn default() -> Self {
    Self::Rule(InstructionsTab::default(), None)
  }
}

impl InstructionsView {
  /// The scope tab a rule or a new rule is in; the import page has none.
  pub const fn tab(self) -> Option<InstructionsTab> {
    match self {
      Self::Rule(tab, _) | Self::New(tab) => Some(tab),
      Self::Import => None,
    }
  }

  pub(super) fn path(self, instructions: String) -> String {
    let named = |tab: InstructionsTab| (tab != InstructionsTab::default()).then(|| tab.name());
    match self {
      Self::Rule(tab, rule) => {
        let rule = rule.map(|id| id.to_string());
        with_query(instructions, &[(TAB, named(tab)), (RULE, rule.as_deref())])
      }
      Self::New(tab) => with_query(instructions, &[(TAB, named(tab)), (RULE, Some(NEW))]),
      Self::Import => with_query(instructions, &[(VIEW, Some(IMPORT))]),
    }
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    match params.take(VIEW)? {
      None => {}
      Some(view) if view == IMPORT => return Ok(Self::Import),
      Some(value) => {
        return Err(RouteError::InvalidParam {
          param: VIEW.to_owned(),
          value,
        });
      }
    }
    let tab = params.take_or_default(TAB, InstructionsTab::from_name)?;
    match params.take(RULE)? {
      None => Ok(Self::Rule(tab, None)),
      Some(rule) if rule == NEW => Ok(Self::New(tab)),
      Some(rule) => match rule.parse() {
        Ok(id) => Ok(Self::Rule(tab, Some(id))),
        Err(_) => Err(RouteError::InvalidParam {
          param: RULE.to_owned(),
          value: rule,
        }),
      },
    }
  }
}
