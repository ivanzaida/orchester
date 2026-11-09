//! A window's Accessibility tree as the driver reads and uses it: each element's facts ([`facts`]), the tree within
//! the snapshot's bounds ([`Walk`]), the elements one after the other for a query or a ref ([`first`]), and an
//! action carried out as [`accessibility::plan`] says, through AX actions and settable attributes only ([`act_on`]).

use orchester_core::orchester_tools::ui::{UiAction, UiElement};

use super::{
  super::{
    NativeUiError, NativeWindow, SEARCH_NODES, SNAPSHOT_DEPTH, SNAPSHOT_NODES,
    accessibility::{self, AxFacts, AxPlan, AxValue, DECREMENT, INCREMENT, MAX_STEPS, PICK, PRESS},
  },
  ax::{AxElement, AxError},
};

/// `AXCancel`: closes a menu that was shown.
const CANCEL: &str = "AXCancel";

impl From<AxError> for NativeUiError {
  fn from(failed: AxError) -> Self {
    Self::Accessibility(failed.to_string())
  }
}

/// The AX window of `window`, found among its app's windows by its window server id.
pub(super) fn window_element(window: &NativeWindow) -> Result<AxElement, NativeUiError> {
  let app = AxElement::application(window.pid).ok_or(NativeUiError::NoWindow)?;
  app
    .elements("AXWindows")?
    .into_iter()
    .find(|element| element.window_id() == Some(window.window_id))
    .ok_or(NativeUiError::NoWindow)
}

/// What the driver reads of `element`.
pub(super) fn facts(element: &AxElement) -> Result<AxFacts, AxError> {
  Ok(AxFacts {
    reference: accessibility::reference(element.hash()),
    role: element.text("AXRole")?.unwrap_or_default(),
    subrole: element.text("AXSubrole")?,
    title: element.text("AXTitle")?,
    description: element.text("AXDescription")?,
    identifier: element.text("AXIdentifier")?,
    value: element.value()?,
    value_settable: element.settable("AXValue")?,
    enabled: element.flag("AXEnabled")?,
    focused: element.flag("AXFocused")?,
    selected: element.flag("AXSelected")?,
    expanded: element.flag("AXExpanded")?,
    actions: element.actions()?,
  })
}

/// One snapshot's walk of a tree: at most [`SNAPSHOT_NODES`] elements, [`SNAPSHOT_DEPTH`] deep.
#[derive(Default)]
pub(super) struct Walk {
  nodes: usize,
  /// Some element was left out for the bounds.
  pub(super) truncated: bool,
}

impl Walk {
  /// `element` with what is inside it; `None` for one that went away while it was read.
  pub(super) fn tree(&mut self, element: &AxElement, depth: usize) -> Option<UiElement> {
    let mut node = facts(element).ok()?.element();
    self.nodes += 1;
    let Ok(inside) = element.elements("AXChildren") else {
      return Some(node);
    };
    for child in inside {
      if self.nodes >= SNAPSHOT_NODES || depth + 1 >= SNAPSHOT_DEPTH {
        self.truncated = true;
        break;
      }
      node.children.extend(self.tree(&child, depth + 1));
    }
    Some(node)
  }
}

/// Visits the elements of `roots` and everything inside them, parents first, until `visit` answers `true` or
/// [`SEARCH_NODES`] were visited; the element it answered `true` for.
pub(super) fn first(
  roots: Vec<AxElement>,
  mut visit: impl FnMut(&AxElement) -> Result<bool, NativeUiError>,
) -> Result<Option<AxElement>, NativeUiError> {
  let mut pending: Vec<AxElement> = roots.into_iter().rev().collect();
  let mut visited = 0;
  while let Some(element) = pending.pop() {
    if visited >= SEARCH_NODES {
      return Ok(None);
    }
    visited += 1;
    if visit(&element)? {
      return Ok(Some(element));
    }
    // An element that went away while it was read has nothing inside it any more.
    if let Ok(inside) = element.elements("AXChildren") {
      pending.extend(inside.into_iter().rev());
    }
  }
  Ok(None)
}

/// Does `action` to `element`, through its AX actions and settable attributes only.
pub(super) fn act_on(element: &AxElement, action: &UiAction) -> Result<(), NativeUiError> {
  let facts = facts(element)?;
  let plan = accessibility::plan(&facts, action)?;
  carry_out(element, &facts, plan)
}

fn carry_out(element: &AxElement, facts: &AxFacts, plan: AxPlan) -> Result<(), NativeUiError> {
  match plan {
    AxPlan::Perform(action) => Ok(element.perform(action)?),
    AxPlan::SetValue(value) => Ok(element.set_value(&value)?),
    AxPlan::StepTo(target) => step_to(element, facts, target),
    AxPlan::Pick {
      option,
      open,
      otherwise,
    } => {
      if pick(element, &option)? {
        return Ok(());
      }
      if let Some(open) = open {
        element.perform(open)?;
        let picked = pick(element, &option);
        if !matches!(picked, Ok(true)) {
          close_menus(element)?;
        }
        if picked? {
          return Ok(());
        }
      }
      carry_out(element, facts, (*otherwise)?)
    }
  }
}

/// Presses, picks or selects the element named `option`, any case, inside `element`; whether one was.
fn pick(element: &AxElement, option: &str) -> Result<bool, NativeUiError> {
  let inside = element.elements("AXChildren")?;
  let named = first(inside, |candidate| {
    Ok(
      facts(candidate)
        .ok()
        .and_then(|facts| facts.name())
        .is_some_and(|name| name.eq_ignore_ascii_case(option)),
    )
  })?;
  let Some(named) = named else {
    return Ok(false);
  };
  let actions = named.actions()?;
  if let Some(action) = [PRESS, PICK]
    .into_iter()
    .find(|name| actions.iter().any(|action| action == name))
  {
    named.perform(action)?;
    return Ok(true);
  }
  if named.settable("AXSelected")? {
    named.set_true("AXSelected")?;
    return Ok(true);
  }
  Ok(false)
}

/// Closes the menus `element` shows, which opening it for a `select` showed.
fn close_menus(element: &AxElement) -> Result<(), NativeUiError> {
  for child in element.elements("AXChildren")? {
    if child.text("AXRole")?.as_deref() == Some("AXMenu") {
      child.perform(CANCEL)?;
    }
  }
  Ok(())
}

/// Increments or decrements `element` until its value is `target`.
fn step_to(element: &AxElement, facts: &AxFacts, target: f64) -> Result<(), NativeUiError> {
  let unreachable = || NativeUiError::NoOption {
    reference: facts.reference.clone(),
    option: target.to_string(),
  };
  let mut direction = None;
  for _ in 0..=MAX_STEPS {
    let Some(AxValue::Number(now)) = element.value()? else {
      return Err(unreachable());
    };
    if (now - target).abs() < f64::EPSILON {
      return Ok(());
    }
    let up = now < target;
    // A step that went past the target and back means the steps do not land on it.
    if direction.is_some_and(|was| was != up) {
      return Err(unreachable());
    }
    direction = Some(up);
    element.perform(if up { INCREMENT } else { DECREMENT })?;
  }
  Err(unreachable())
}
