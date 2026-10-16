//! What Instructions shows (`fePhH`): its scope tab and the rule open in it, a new rule, or the import page
//! (`/project/<id>/instructions?tab=roles&rule=…`, `&rule=new`, `?view=import`), with the panel beside a
//! rule (`&panel=preview`; left out, the tab's own).

use orchester_core::id::ResourceId;

use super::{
  RouteError, TAB,
  query::{Params, with_query},
};

const RULE: &str = "rule";
const VIEW: &str = "view";
const PANEL: &str = "panel";
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

  /// The panel a rule of the tab opens with: none for a role, which has no variables of a run.
  pub const fn panel(self) -> InstructionsPanel {
    match self {
      Self::Roles => InstructionsPanel::Hidden,
      Self::Steps => InstructionsPanel::Preview,
      Self::Project | Self::Pipelines => InstructionsPanel::Variables,
    }
  }
}

/// What is beside a rule: nothing, its Variables or its Preview (`r8Ipz`), or the full prompt in place of
/// the page (`fTuZZ`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InstructionsPanel {
  Hidden,
  Variables,
  Preview,
  Full,
}

impl lurq::app::component::DevtoolsInspectable for InstructionsPanel {}

impl InstructionsPanel {
  const ALL: [Self; 4] = [Self::Hidden, Self::Variables, Self::Preview, Self::Full];

  const fn name(self) -> &'static str {
    match self {
      Self::Hidden => "none",
      Self::Variables => "variables",
      Self::Preview => "preview",
      Self::Full => "prompt",
    }
  }

  fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|panel| panel.name() == name)
  }
}

/// A rule of a tab (`None`: the tab's first), a new rule in a tab, or the import page; a rule with its panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InstructionsView {
  Rule(InstructionsTab, Option<ResourceId>, InstructionsPanel),
  New(InstructionsTab, InstructionsPanel),
  Import,
}

// The view a person left Instructions for, held while unsaved edits are asked about.
impl lurq::app::component::DevtoolsInspectable for InstructionsView {}

impl Default for InstructionsView {
  fn default() -> Self {
    Self::rule(InstructionsTab::default(), None)
  }
}

impl InstructionsView {
  /// `rule` of `tab` (`None`: its first), with the tab's panel.
  pub const fn rule(tab: InstructionsTab, rule: Option<ResourceId>) -> Self {
    Self::Rule(tab, rule, tab.panel())
  }

  /// A new rule in `tab`, with the tab's panel.
  pub const fn new_rule(tab: InstructionsTab) -> Self {
    Self::New(tab, tab.panel())
  }

  /// The scope tab a rule or a new rule is in; the import page has none.
  pub const fn tab(self) -> Option<InstructionsTab> {
    match self {
      Self::Rule(tab, ..) | Self::New(tab, _) => Some(tab),
      Self::Import => None,
    }
  }

  /// The panel beside a rule or a new rule; the import page has none.
  pub const fn panel(self) -> Option<InstructionsPanel> {
    match self {
      Self::Rule(.., panel) | Self::New(_, panel) => Some(panel),
      Self::Import => None,
    }
  }

  /// The same rule with `panel` beside it; the import page as it is.
  pub const fn with_panel(self, panel: InstructionsPanel) -> Self {
    match self {
      Self::Rule(tab, rule, _) => Self::Rule(tab, rule, panel),
      Self::New(tab, _) => Self::New(tab, panel),
      Self::Import => Self::Import,
    }
  }

  pub(super) fn path(self, instructions: String) -> String {
    let named = |tab: InstructionsTab| (tab != InstructionsTab::default()).then(|| tab.name());
    let panel = |tab: InstructionsTab, panel: InstructionsPanel| (panel != tab.panel()).then(|| panel.name());
    match self {
      Self::Rule(tab, rule, shown) => {
        let rule = rule.map(|id| id.to_string());
        with_query(
          instructions,
          &[(TAB, named(tab)), (RULE, rule.as_deref()), (PANEL, panel(tab, shown))],
        )
      }
      Self::New(tab, shown) => with_query(
        instructions,
        &[(TAB, named(tab)), (RULE, Some(NEW)), (PANEL, panel(tab, shown))],
      ),
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
    let view = match params.take(RULE)? {
      None => Self::rule(tab, None),
      Some(rule) if rule == NEW => Self::new_rule(tab),
      Some(rule) => match rule.parse() {
        Ok(id) => Self::rule(tab, Some(id)),
        Err(_) => {
          return Err(RouteError::InvalidParam {
            param: RULE.to_owned(),
            value: rule,
          });
        }
      },
    };
    match params.take(PANEL)? {
      None => Ok(view),
      Some(panel) => match InstructionsPanel::from_name(&panel) {
        Some(panel) => Ok(view.with_panel(panel)),
        None => Err(RouteError::InvalidParam {
          param: PANEL.to_owned(),
          value: panel,
        }),
      },
    }
  }
}
