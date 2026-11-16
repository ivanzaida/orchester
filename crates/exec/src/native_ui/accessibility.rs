//! macOS Accessibility (AX) as the native driver shows and uses it, apart from the platform so it is tested on every
//! one: what an element's attributes make of it in a snapshot ([`AxFacts::element`]), and how an action is done
//! through the element's own AX actions and settable attributes only ([`plan`]) — or why it cannot be. The macOS
//! driver reads the facts and carries the plan out.

use orchester_core::orchester_tools::ui::{UiAction, UiElement};

use super::NativeUiError;

/// The AX roles and the snapshot role each is shown as; the words of the Windows driver where one fits.
const ROLES: [(&str, &str); 40] = [
  ("AXApplication", "application"),
  ("AXBrowser", "tree"),
  ("AXBusyIndicator", "progressbar"),
  ("AXButton", "button"),
  ("AXCell", "dataitem"),
  ("AXCheckBox", "checkbox"),
  ("AXColumn", "headeritem"),
  ("AXComboBox", "combobox"),
  ("AXDisclosureTriangle", "button"),
  ("AXGrid", "datagrid"),
  ("AXGroup", "group"),
  ("AXHeading", "header"),
  ("AXImage", "image"),
  ("AXIncrementor", "spinner"),
  ("AXLink", "hyperlink"),
  ("AXList", "list"),
  ("AXMenu", "menu"),
  ("AXMenuBar", "menubar"),
  ("AXMenuBarItem", "menuitem"),
  ("AXMenuButton", "button"),
  ("AXMenuItem", "menuitem"),
  ("AXOutline", "tree"),
  ("AXPopUpButton", "combobox"),
  ("AXProgressIndicator", "progressbar"),
  ("AXRadioButton", "radiobutton"),
  ("AXRadioGroup", "group"),
  ("AXRow", "listitem"),
  ("AXScrollArea", "pane"),
  ("AXScrollBar", "scrollbar"),
  ("AXSlider", "slider"),
  ("AXSplitGroup", "pane"),
  ("AXSplitter", "separator"),
  ("AXStaticText", "text"),
  ("AXTabGroup", "tab"),
  ("AXTable", "table"),
  ("AXTextArea", "edit"),
  ("AXTextField", "edit"),
  ("AXToolbar", "toolbar"),
  ("AXWebArea", "document"),
  ("AXWindow", "window"),
];

/// The roles whose value is text a person types.
const TEXT_ROLES: [&str; 3] = ["AXTextField", "AXTextArea", "AXComboBox"];
/// The roles whose value is their checked state: 0, 1, or 2 for mixed.
const CHECK_ROLES: [&str; 2] = ["AXCheckBox", "AXRadioButton"];
/// The roles that hold options to select among.
const OPTION_ROLES: [&str; 7] = [
  "AXPopUpButton",
  "AXComboBox",
  "AXList",
  "AXTable",
  "AXOutline",
  "AXRadioGroup",
  "AXTabGroup",
];
/// The subrole of a password field, whose text is never shown.
const SECURE: &str = "AXSecureTextField";
/// The most `AXIncrement`/`AXDecrement` steps a `select` takes towards a number.
pub const MAX_STEPS: usize = 100;

pub const PRESS: &str = "AXPress";
pub const PICK: &str = "AXPick";
pub const SHOW_MENU: &str = "AXShowMenu";
pub const SCROLL_TO_VISIBLE: &str = "AXScrollToVisible";
pub const INCREMENT: &str = "AXIncrement";
pub const DECREMENT: &str = "AXDecrement";

/// What an element's `AXValue` holds.
#[derive(Clone, Debug, PartialEq)]
pub enum AxValue {
  Text(String),
  Number(f64),
}

/// An element as AX describes it: what the driver reads of it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AxFacts {
  /// Its ref ([`reference`]).
  pub reference: String,
  /// `AXRole`, such as `AXButton`.
  pub role: String,
  pub subrole: Option<String>,
  pub title: Option<String>,
  /// `AXDescription`, which names an element without a title, such as an image button.
  pub description: Option<String>,
  /// `AXIdentifier`, which a query also looks in.
  pub identifier: Option<String>,
  pub value: Option<AxValue>,
  /// Whether `AXValue` may be set.
  pub value_settable: bool,
  pub enabled: Option<bool>,
  pub focused: Option<bool>,
  pub selected: Option<bool>,
  pub expanded: Option<bool>,
  /// The names of the AX actions it performs, such as `AXPress`.
  pub actions: Vec<String>,
}

/// The ref of the element whose `CFHash` is `hash`: AX gives an element no id, and its hash, which follows its
/// equality, stays the same while the element lives, however often it is read.
pub fn reference(hash: u64) -> String {
  format!("x{hash:x}")
}

/// The snapshot role of the AX role `role`: the Windows driver's word for it where one fits, else the AX role without
/// its `AX`, in lower case (`AXValueIndicator` is `valueindicator`).
pub fn role(role: &str) -> String {
  ROLES.iter().find(|(ax, _)| *ax == role).map_or_else(
    || role.strip_prefix("AX").unwrap_or(role).to_lowercase(),
    |(_, shown)| (*shown).to_owned(),
  )
}

impl AxFacts {
  fn is(&self, roles: &[&str]) -> bool {
    roles.contains(&self.role.as_str())
  }

  fn password(&self) -> bool {
    self.subrole.as_deref() == Some(SECURE)
  }

  fn can(&self, action: &str) -> bool {
    self.actions.iter().any(|name| name == action)
  }

  fn text(&self) -> Option<&str> {
    match &self.value {
      Some(AxValue::Text(text)) if !text.is_empty() => Some(text),
      _ => None,
    }
  }

  /// Its name: its title, else its description, else the text of a label, whose value is what it says.
  pub fn name(&self) -> Option<String> {
    let label = (self.role == "AXStaticText").then(|| self.text()).flatten();
    [self.title.as_deref(), self.description.as_deref(), label]
      .into_iter()
      .flatten()
      .find(|text| !text.is_empty())
      .map(str::to_owned)
  }

  /// Its states, in the Windows driver's words.
  pub fn states(&self) -> Vec<String> {
    let mut states = Vec::new();
    if self.enabled == Some(false) {
      states.push("disabled");
    }
    if self.focused == Some(true) {
      states.push("focused");
    }
    if self.is(&CHECK_ROLES)
      && let Some(AxValue::Number(checked)) = self.value
    {
      states.push(match checked {
        on if on >= 2.0 => "mixed",
        on if on >= 1.0 => "checked",
        _ => "unchecked",
      });
    }
    match self.expanded {
      Some(true) => states.push("expanded"),
      Some(false) => states.push("collapsed"),
      None => {}
    }
    if self.selected == Some(true) {
      states.push("selected");
    }
    if self.is(&TEXT_ROLES) && !self.value_settable {
      states.push("readonly");
    }
    if self.password() {
      states.push("password");
    }
    states.into_iter().map(str::to_owned).collect()
  }

  /// Its value as a snapshot shows it: never a password field's, never a checked state or a label's text, which
  /// the states and the name already say.
  pub fn shown_value(&self) -> Option<String> {
    if self.password() || self.is(&CHECK_ROLES) || self.role == "AXStaticText" {
      return None;
    }
    match &self.value {
      Some(AxValue::Text(text)) => (!text.is_empty()).then(|| text.clone()),
      Some(AxValue::Number(number)) => Some(number.to_string()),
      None => None,
    }
  }

  /// It as a snapshot shows it, without its children.
  pub fn element(&self) -> UiElement {
    UiElement {
      reference: self.reference.clone(),
      role: role(&self.role),
      name: self.name(),
      states: self.states(),
      value: self.shown_value(),
      children: Vec::new(),
    }
  }

  /// Whether a query, already in lower case, matches it: its name, identifier or AX role holds it.
  pub fn matches(&self, query: &str) -> bool {
    [self.name(), self.identifier.clone(), Some(self.role.clone())]
      .into_iter()
      .flatten()
      .any(|text| text.to_lowercase().contains(query))
  }

  fn unsupported(&self, action: &'static str, needs: &'static str) -> NativeUiError {
    NativeUiError::Unsupported {
      action,
      needs,
      reference: self.reference.clone(),
      role: role(&self.role),
    }
  }
}

/// How the driver does an action to an element, through AX only.
#[derive(Clone, Debug, PartialEq)]
pub enum AxPlan {
  /// Perform the AX action of this name.
  Perform(&'static str),
  /// Set `AXValue` to this.
  SetValue(AxValue),
  /// Perform `AXIncrement` or `AXDecrement` until the value is this, at most [`MAX_STEPS`] times.
  StepTo(f64),
  /// Press or pick the element named `option`, any case, inside it — opening it first with the action `open`
  /// when it has one and the option is not shown — and else do `otherwise`.
  Pick {
    option: String,
    open: Option<&'static str>,
    otherwise: Box<Result<AxPlan, NativeUiError>>,
  },
}

/// How `action` is done to the element `facts` describes, or why it cannot be.
pub fn plan(facts: &AxFacts, action: &UiAction) -> Result<AxPlan, NativeUiError> {
  match action {
    UiAction::Click => [PRESS, PICK]
      .into_iter()
      .find(|name| facts.can(name))
      .map(AxPlan::Perform)
      .ok_or_else(|| facts.unsupported("click", "the AXPress or AXPick action")),
    UiAction::Type(text) => match (&facts.value, facts.value_settable) {
      (_, true) => Ok(AxPlan::SetValue(AxValue::Text(text.clone()))),
      (Some(AxValue::Text(_)), false) => Err(NativeUiError::ReadOnly(facts.reference.clone())),
      _ => Err(facts.unsupported("type", "a settable AXValue")),
    },
    UiAction::Key(_) => Err(NativeUiError::NoKeys),
    UiAction::Scroll => {
      if facts.can(SCROLL_TO_VISIBLE) {
        Ok(AxPlan::Perform(SCROLL_TO_VISIBLE))
      } else {
        Err(facts.unsupported("scroll", "the AXScrollToVisible action"))
      }
    }
    UiAction::Select(option) => Ok(AxPlan::Pick {
      option: option.clone(),
      open: opener(facts),
      otherwise: Box::new(set_to(facts, option)),
    }),
  }
}

/// The action that shows the options of an element that holds them out of sight, such as a pop-up button's menu.
fn opener(facts: &AxFacts) -> Option<&'static str> {
  if facts.can(SHOW_MENU) {
    return Some(SHOW_MENU);
  }
  (facts.is(&["AXPopUpButton", "AXMenuButton"]) && facts.can(PRESS)).then_some(PRESS)
}

/// How a `select` of `option` is done when no element inside is named so: the value set to it, or stepped to it.
fn set_to(facts: &AxFacts, option: &str) -> Result<AxPlan, NativeUiError> {
  let number = option.trim().parse::<f64>().ok().filter(|number| number.is_finite());
  let no_option = || NativeUiError::NoOption {
    reference: facts.reference.clone(),
    option: option.to_owned(),
  };
  match (&facts.value, facts.value_settable, number) {
    (Some(AxValue::Number(_)), true, Some(number)) => Ok(AxPlan::SetValue(AxValue::Number(number))),
    (Some(AxValue::Number(_)), true, None) => Err(no_option()),
    (_, true, _) => Ok(AxPlan::SetValue(AxValue::Text(option.to_owned()))),
    (Some(AxValue::Number(_)), false, Some(number)) if facts.can(INCREMENT) && facts.can(DECREMENT) => {
      Ok(AxPlan::StepTo(number))
    }
    (Some(AxValue::Text(_)), false, _) if facts.is(&TEXT_ROLES) => {
      Err(NativeUiError::ReadOnly(facts.reference.clone()))
    }
    _ if opener(facts).is_some() || facts.is(&OPTION_ROLES) => Err(no_option()),
    _ => Err(facts.unsupported(
      "select",
      "an option inside it with AXPress or AXPick, a settable AXValue, or AXIncrement and AXDecrement",
    )),
  }
}
