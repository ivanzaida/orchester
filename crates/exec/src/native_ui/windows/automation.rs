#![allow(unsafe_code)]
//! UI Automation's COM client has no safe wrapper: this module holds the COM apartment of the thread that uses it,
//! the automation object, and each element with the properties and patterns the driver reads and uses. A pattern an
//! element does not have comes back as `None`, never as an error. Everything stays on the thread that made it.

use std::ffi::c_void;

use windows::{
  Win32::{
    Foundation::{HWND, RECT},
    System::{
      Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize, SAFEARRAY},
      Ole::{SafeArrayDestroy, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound},
    },
    UI::Accessibility::{
      CUIAutomation, ExpandCollapseState, ExpandCollapseState_Collapsed, ExpandCollapseState_Expanded,
      ExpandCollapseState_PartiallyExpanded, IUIAutomation, IUIAutomationElement, IUIAutomationExpandCollapsePattern,
      IUIAutomationInvokePattern, IUIAutomationScrollItemPattern, IUIAutomationSelectionItemPattern,
      IUIAutomationTogglePattern, IUIAutomationTreeWalker, IUIAutomationValuePattern, ToggleState_Indeterminate,
      ToggleState_On, UIA_ExpandCollapsePatternId, UIA_InvokePatternId, UIA_PATTERN_ID, UIA_ScrollItemPatternId,
      UIA_SelectionItemPatternId, UIA_TogglePatternId, UIA_ValuePatternId,
    },
  },
  core::{BSTR, Interface},
};

/// What a call that failed says.
type Failed = String;

fn failed(error: &windows::core::Error) -> Failed {
  error.message()
}

/// `result` as `None` when it holds no object: UI Automation answers a missing element or pattern with success and
/// nothing in it.
fn present<T>(result: windows::core::Result<T>) -> Result<Option<T>, Failed> {
  match result {
    Ok(value) => Ok(Some(value)),
    Err(error) if error.code().is_ok() => Ok(None),
    Err(error) => Err(failed(&error)),
  }
}

/// The COM apartment of this thread, left when it drops.
struct Apartment;

impl Drop for Apartment {
  fn drop(&mut self) {
    // SAFETY: pairs the successful CoInitializeEx of `Automation::new` on this same thread.
    unsafe { CoUninitialize() };
  }
}

/// UI Automation on this thread. Its objects are released before the apartment is left (fields drop in order).
pub(super) struct Automation {
  automation: IUIAutomation,
  walker: IUIAutomationTreeWalker,
  _apartment: Apartment,
}

impl Automation {
  /// UI Automation in a multithreaded apartment of this thread, which must not have joined another one.
  pub(super) fn new() -> Result<Self, Failed> {
    // SAFETY: no reserved pointer; the apartment is left by `Apartment`'s drop on this thread.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
      .ok()
      .map_err(|error| failed(&error))?;
    let apartment = Apartment;
    // SAFETY: the documented class of the automation object, created in this process.
    let automation: IUIAutomation =
      unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.map_err(|error| failed(&error))?;
    // SAFETY: a plain call on the live automation object.
    let walker = unsafe { automation.ControlViewWalker() }.map_err(|error| failed(&error))?;
    Ok(Self {
      automation,
      walker,
      _apartment: apartment,
    })
  }

  /// The element of the window `handle`.
  pub(super) fn window(&self, handle: isize) -> Result<Element, Failed> {
    // SAFETY: the handle is of a window of a checked process; UI Automation fails for one that is gone.
    unsafe { self.automation.ElementFromHandle(HWND(handle as *mut c_void)) }
      .map(Element)
      .map_err(|error| failed(&error))
  }

  /// The first element inside `element` in the control view.
  pub(super) fn first_child(&self, element: &Element) -> Result<Option<Element>, Failed> {
    // SAFETY: a plain call on live objects.
    present(unsafe { self.walker.GetFirstChildElement(&element.0) }).map(|child| child.map(Element))
  }

  /// The element after `element` among its siblings in the control view.
  pub(super) fn next_sibling(&self, element: &Element) -> Result<Option<Element>, Failed> {
    // SAFETY: a plain call on live objects.
    present(unsafe { self.walker.GetNextSiblingElement(&element.0) }).map(|next| next.map(Element))
  }
}

/// How a toggle stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Toggled {
  On,
  Off,
  Mixed,
}

/// Whether an element that expands is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Expanded {
  Open,
  Closed,
  /// It has nothing to expand.
  Leaf,
}

/// An element of a window's UI Automation tree.
#[derive(Clone)]
pub(super) struct Element(IUIAutomationElement);

impl Element {
  fn pattern<T: Interface>(&self, id: UIA_PATTERN_ID) -> Result<Option<T>, Failed> {
    // SAFETY: the pattern id and the interface asked for belong together, as each caller names them.
    present(unsafe { self.0.GetCurrentPatternAs::<T>(id) })
  }

  /// Its runtime id: unique on the desktop while the element exists.
  pub(super) fn runtime_id(&self) -> Result<Vec<i32>, Failed> {
    // SAFETY: a plain call; the array it answers is owned here and destroyed below.
    let array = unsafe { self.0.GetRuntimeId() }.map_err(|error| failed(&error))?;
    if array.is_null() {
      return Ok(Vec::new());
    }
    let read = read_ints(array);
    // SAFETY: the array came from GetRuntimeId, is owned here and destroyed once.
    unsafe { SafeArrayDestroy(array) }.map_err(|error| failed(&error))?;
    read
  }

  pub(super) fn control_type(&self) -> Result<i32, Failed> {
    // SAFETY: a plain property read.
    unsafe { self.0.CurrentControlType() }
      .map(|id| id.0)
      .map_err(|error| failed(&error))
  }

  pub(super) fn name(&self) -> Result<String, Failed> {
    // SAFETY: a plain property read.
    text(unsafe { self.0.CurrentName() })
  }

  pub(super) fn automation_id(&self) -> Result<String, Failed> {
    // SAFETY: a plain property read.
    text(unsafe { self.0.CurrentAutomationId() })
  }

  pub(super) fn class_name(&self) -> Result<String, Failed> {
    // SAFETY: a plain property read.
    text(unsafe { self.0.CurrentClassName() })
  }

  pub(super) fn enabled(&self) -> Result<bool, Failed> {
    // SAFETY: a plain property read.
    flag(unsafe { self.0.CurrentIsEnabled() })
  }

  pub(super) fn focused(&self) -> Result<bool, Failed> {
    // SAFETY: a plain property read.
    flag(unsafe { self.0.CurrentHasKeyboardFocus() })
  }

  pub(super) fn password(&self) -> Result<bool, Failed> {
    // SAFETY: a plain property read.
    flag(unsafe { self.0.CurrentIsPassword() })
  }

  /// Where it is on the desktop: left, top, right, bottom.
  pub(super) fn rectangle(&self) -> Result<(i32, i32, i32, i32), Failed> {
    // SAFETY: a plain property read.
    let RECT {
      left,
      top,
      right,
      bottom,
    } = unsafe { self.0.CurrentBoundingRectangle() }.map_err(|error| failed(&error))?;
    Ok((left, top, right, bottom))
  }

  /// Invokes it; `false` without the Invoke pattern.
  pub(super) fn invoke(&self) -> Result<bool, Failed> {
    let Some(pattern) = self.pattern::<IUIAutomationInvokePattern>(UIA_InvokePatternId)? else {
      return Ok(false);
    };
    // SAFETY: a plain call on the element's own pattern.
    unsafe { pattern.Invoke() }.map_err(|error| failed(&error))?;
    Ok(true)
  }

  /// How it is toggled; `None` without the Toggle pattern.
  pub(super) fn toggled(&self) -> Result<Option<Toggled>, Failed> {
    let Some(pattern) = self.pattern::<IUIAutomationTogglePattern>(UIA_TogglePatternId)? else {
      return Ok(None);
    };
    // SAFETY: a plain property read of the element's own pattern.
    let state = unsafe { pattern.CurrentToggleState() }.map_err(|error| failed(&error))?;
    Ok(Some(if state == ToggleState_On {
      Toggled::On
    } else if state == ToggleState_Indeterminate {
      Toggled::Mixed
    } else {
      Toggled::Off
    }))
  }

  /// Toggles it; `false` without the Toggle pattern.
  pub(super) fn toggle(&self) -> Result<bool, Failed> {
    let Some(pattern) = self.pattern::<IUIAutomationTogglePattern>(UIA_TogglePatternId)? else {
      return Ok(false);
    };
    // SAFETY: a plain call on the element's own pattern.
    unsafe { pattern.Toggle() }.map_err(|error| failed(&error))?;
    Ok(true)
  }

  /// Whether it is expanded; `None` without the ExpandCollapse pattern.
  pub(super) fn expanded(&self) -> Result<Option<Expanded>, Failed> {
    let Some(pattern) = self.expand_collapse()? else {
      return Ok(None);
    };
    // SAFETY: a plain property read of the element's own pattern.
    let state: ExpandCollapseState = unsafe { pattern.CurrentExpandCollapseState() }.map_err(|error| failed(&error))?;
    Ok(Some(
      if state == ExpandCollapseState_Expanded || state == ExpandCollapseState_PartiallyExpanded {
        Expanded::Open
      } else if state == ExpandCollapseState_Collapsed {
        Expanded::Closed
      } else {
        Expanded::Leaf
      },
    ))
  }

  /// Expands it when `open`, else collapses it; `false` without the ExpandCollapse pattern.
  pub(super) fn expand(&self, open: bool) -> Result<bool, Failed> {
    let Some(pattern) = self.expand_collapse()? else {
      return Ok(false);
    };
    // SAFETY: plain calls on the element's own pattern.
    let done = if open {
      unsafe { pattern.Expand() }
    } else {
      unsafe { pattern.Collapse() }
    };
    done.map_err(|error| failed(&error))?;
    Ok(true)
  }

  fn expand_collapse(&self) -> Result<Option<IUIAutomationExpandCollapsePattern>, Failed> {
    self.pattern::<IUIAutomationExpandCollapsePattern>(UIA_ExpandCollapsePatternId)
  }

  /// Whether it is selected; `None` without the SelectionItem pattern.
  pub(super) fn selected(&self) -> Result<Option<bool>, Failed> {
    let Some(pattern) = self.pattern::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId)? else {
      return Ok(None);
    };
    // SAFETY: a plain property read of the element's own pattern.
    flag(unsafe { pattern.CurrentIsSelected() }).map(Some)
  }

  /// Selects it; `false` without the SelectionItem pattern.
  pub(super) fn select(&self) -> Result<bool, Failed> {
    let Some(pattern) = self.pattern::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId)? else {
      return Ok(false);
    };
    // SAFETY: a plain call on the element's own pattern.
    unsafe { pattern.Select() }.map_err(|error| failed(&error))?;
    Ok(true)
  }

  /// Its value and whether it is read-only; `None` without the Value pattern.
  pub(super) fn value(&self) -> Result<Option<(String, bool)>, Failed> {
    let Some(pattern) = self.value_pattern()? else {
      return Ok(None);
    };
    // SAFETY: plain property reads of the element's own pattern.
    let value = text(unsafe { pattern.CurrentValue() })?;
    // SAFETY: as above.
    let read_only = flag(unsafe { pattern.CurrentIsReadOnly() })?;
    Ok(Some((value, read_only)))
  }

  /// Sets its value to `value`; `false` without the Value pattern.
  pub(super) fn set_value(&self, value: &str) -> Result<bool, Failed> {
    let Some(pattern) = self.value_pattern()? else {
      return Ok(false);
    };
    // SAFETY: a plain call on the element's own pattern with a string it copies.
    unsafe { pattern.SetValue(&BSTR::from(value)) }.map_err(|error| failed(&error))?;
    Ok(true)
  }

  fn value_pattern(&self) -> Result<Option<IUIAutomationValuePattern>, Failed> {
    self.pattern::<IUIAutomationValuePattern>(UIA_ValuePatternId)
  }

  /// Scrolls it into view; `false` without the ScrollItem pattern.
  pub(super) fn scroll_into_view(&self) -> Result<bool, Failed> {
    let Some(pattern) = self.pattern::<IUIAutomationScrollItemPattern>(UIA_ScrollItemPatternId)? else {
      return Ok(false);
    };
    // SAFETY: a plain call on the element's own pattern.
    unsafe { pattern.ScrollIntoView() }.map_err(|error| failed(&error))?;
    Ok(true)
  }
}

fn text(result: windows::core::Result<BSTR>) -> Result<String, Failed> {
  result.map(|text| text.to_string()).map_err(|error| failed(&error))
}

fn flag(result: windows::core::Result<windows::core::BOOL>) -> Result<bool, Failed> {
  result.map(|value| value.as_bool()).map_err(|error| failed(&error))
}

/// The 32-bit integers of the one-dimensional `array`.
fn read_ints(array: *mut SAFEARRAY) -> Result<Vec<i32>, Failed> {
  // SAFETY: the array is live and one-dimensional, as a runtime id is.
  let (low, high) = unsafe { (SafeArrayGetLBound(array, 1), SafeArrayGetUBound(array, 1)) };
  let (low, high) = (
    low.map_err(|error| failed(&error))?,
    high.map_err(|error| failed(&error))?,
  );
  let mut ints = Vec::new();
  for index in low..=high {
    let mut value = 0i32;
    // SAFETY: the index lies within the bounds read above, and value is a writable i32, the array's element type.
    unsafe { SafeArrayGetElement(array, &index, (&raw mut value).cast()) }.map_err(|error| failed(&error))?;
    ints.push(value);
  }
  Ok(ints)
}
