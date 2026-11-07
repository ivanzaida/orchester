//! A window's UI Automation tree as the driver reads and uses it: each element described with its ref, role, name,
//! states and value ([`describe`]), the tree within the snapshot's bounds ([`Walk`]), every element of it one after
//! the other for a query or a ref ([`each`]), and the action a call asks done through the element's patterns only
//! ([`act_on`]).

use orchester_core::orchester_tools::ui::{MAX_REF_CHARS, UiAction, UiElement};

use super::{
  super::{NativeUiError, SEARCH_NODES, SNAPSHOT_DEPTH, SNAPSHOT_NODES},
  automation::{Automation, Element, Expanded, Toggled},
  roles,
};

impl From<String> for NativeUiError {
  fn from(failed: String) -> Self {
    Self::Automation(failed)
  }
}

/// The ref of the element whose runtime id is `ids`: `u` and the ids joined by `.`, or, for one too long for a ref,
/// `h` and a hash of them.
pub(super) fn reference(ids: &[i32]) -> String {
  let joined: Vec<String> = ids.iter().map(i32::to_string).collect();
  let reference = format!("u{}", joined.join("."));
  if reference.len() <= MAX_REF_CHARS {
    return reference;
  }
  // FNV-1a: the same ids give the same ref in every call.
  let hash = ids
    .iter()
    .flat_map(|id| id.to_le_bytes())
    .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
      (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
  format!("h{hash:016x}")
}

/// An element as a snapshot shows it, without its children, and its automation id and class, which a query also
/// looks in.
pub(super) struct Described {
  pub(super) element: UiElement,
  pub(super) automation_id: String,
  pub(super) class: String,
}

pub(super) fn describe(element: &Element) -> Result<Described, String> {
  let mut states = Vec::new();
  if !element.enabled()? {
    states.push("disabled");
  }
  if element.focused()? {
    states.push("focused");
  }
  match element.toggled()? {
    Some(Toggled::On) => states.push("checked"),
    Some(Toggled::Off) => states.push("unchecked"),
    Some(Toggled::Mixed) => states.push("mixed"),
    None => {}
  }
  match element.expanded()? {
    Some(Expanded::Open) => states.push("expanded"),
    Some(Expanded::Closed) => states.push("collapsed"),
    Some(Expanded::Leaf) | None => {}
  }
  if element.selected()? == Some(true) {
    states.push("selected");
  }
  let password = element.password()?;
  let mut value = None;
  if let Some((text, read_only)) = element.value()? {
    if read_only {
      states.push("readonly");
    }
    // A password field's text is never shown.
    if password {
      states.push("password");
    } else if !text.is_empty() {
      value = Some(text);
    }
  }
  let name = element.name()?;
  Ok(Described {
    element: UiElement {
      reference: reference(&element.runtime_id()?),
      role: roles::of(element.control_type()?).to_owned(),
      name: (!name.is_empty()).then_some(name),
      states: states.into_iter().map(str::to_owned).collect(),
      value,
      children: Vec::new(),
    },
    automation_id: element.automation_id()?,
    class: element.class_name()?,
  })
}

/// The elements directly inside `element`, in order.
fn children(automation: &Automation, element: &Element) -> Result<Vec<Element>, String> {
  let mut children = Vec::new();
  let mut next = automation.first_child(element)?;
  while let Some(child) = next {
    next = automation.next_sibling(&child)?;
    children.push(child);
  }
  Ok(children)
}

/// One snapshot's walk of a tree: at most [`SNAPSHOT_NODES`] elements, [`SNAPSHOT_DEPTH`] deep.
pub(super) struct Walk<'a> {
  pub(super) automation: &'a Automation,
  pub(super) nodes: usize,
  /// Some element was left out for the bounds.
  pub(super) truncated: bool,
}

impl Walk<'_> {
  /// `element` with what is inside it; `None` for one that went away while it was read.
  pub(super) fn tree(&mut self, element: &Element, depth: usize) -> Option<UiElement> {
    let mut node = describe(element).ok()?.element;
    self.nodes += 1;
    let Ok(inside) = children(self.automation, element) else {
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
/// [`SEARCH_NODES`] were visited; whether `visit` answered `true`.
pub(super) fn each(
  automation: &Automation,
  roots: Vec<Element>,
  mut visit: impl FnMut(&Element) -> Result<bool, NativeUiError>,
) -> Result<bool, NativeUiError> {
  let mut pending: Vec<Element> = roots.into_iter().rev().collect();
  let mut visited = 0;
  while let Some(element) = pending.pop() {
    if visited >= SEARCH_NODES {
      return Ok(false);
    }
    visited += 1;
    if visit(&element)? {
      return Ok(true);
    }
    // An element that went away while it was read has nothing inside it any more.
    if let Ok(inside) = children(automation, &element) {
      pending.extend(inside.into_iter().rev());
    }
  }
  Ok(false)
}

/// Does `action` to `element`, whose ref is `reference`, through its patterns only.
pub(super) fn act_on(
  automation: &Automation,
  (element, reference): (Element, &str),
  action: &UiAction,
) -> Result<(), NativeUiError> {
  let unsupported = |action: &'static str, needs: &'static str| NativeUiError::Unsupported {
    action,
    needs,
    reference: reference.to_owned(),
    role: element
      .control_type()
      .map_or_else(|_| "unknown".to_owned(), |id| roles::of(id).to_owned()),
  };
  match action {
    UiAction::Click => {
      let clicked = element.invoke()? || element.toggle()? || element.select()? || expand_or_collapse(&element)?;
      if clicked {
        Ok(())
      } else {
        Err(unsupported(
          "click",
          "the Invoke, Toggle, SelectionItem or ExpandCollapse pattern",
        ))
      }
    }
    UiAction::Type(text) => match element.value()? {
      None => Err(unsupported("type", "the Value pattern")),
      Some((_, true)) => Err(NativeUiError::ReadOnly(reference.to_owned())),
      Some((_, false)) => element.set_value(text).map(drop).map_err(NativeUiError::from),
    },
    UiAction::Key(_) => Err(NativeUiError::NoKeys),
    UiAction::Scroll => {
      if element.scroll_into_view()? {
        Ok(())
      } else {
        Err(unsupported("scroll", "the ScrollItem pattern"))
      }
    }
    UiAction::Select(option) => {
      if select_option(automation, (&element, reference), option)? {
        return Ok(());
      }
      Err(unsupported(
        "select",
        "the SelectionItem (of an option inside it), ExpandCollapse or Value pattern",
      ))
    }
  }
}

/// Opens `element` when it is closed, else closes it; `false` when it has nothing to expand.
fn expand_or_collapse(element: &Element) -> Result<bool, String> {
  match element.expanded()? {
    Some(Expanded::Closed) => element.expand(true),
    Some(Expanded::Open) => element.expand(false),
    Some(Expanded::Leaf) | None => Ok(false),
  }
}

/// Selects the option named `option` inside `element`, opening it first when it is closed and closing it again; else
/// sets the value of a field to it. `false` when the element has no pattern for either.
fn select_option(
  automation: &Automation,
  (element, reference): (&Element, &str),
  option: &str,
) -> Result<bool, NativeUiError> {
  if pick(automation, element, option)? {
    return Ok(true);
  }
  let expanded = element.expanded()?;
  if expanded == Some(Expanded::Closed) {
    element.expand(true)?;
    let picked = pick(automation, element, option);
    element.expand(false)?;
    if picked? {
      return Ok(true);
    }
  }
  match element.value()? {
    Some((_, true)) => Err(NativeUiError::ReadOnly(reference.to_owned())),
    Some((_, false)) => Ok(element.set_value(option)?),
    None if expanded.is_some() || has_options(automation, element)? => Err(NativeUiError::NoOption {
      reference: reference.to_owned(),
      option: option.to_owned(),
    }),
    None => Ok(false),
  }
}

/// Selects the element named `option`, any case, with the SelectionItem pattern inside `element`; whether one was.
fn pick(automation: &Automation, element: &Element, option: &str) -> Result<bool, NativeUiError> {
  let inside = children(automation, element)?;
  each(automation, inside, |candidate| {
    Ok(candidate.name()?.eq_ignore_ascii_case(option) && candidate.select()?)
  })
}

/// Whether something inside `element` can be selected.
fn has_options(automation: &Automation, element: &Element) -> Result<bool, NativeUiError> {
  let inside = children(automation, element)?;
  each(automation, inside, |candidate| Ok(candidate.selected()?.is_some()))
}
