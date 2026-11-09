//! The native driver on Windows: the run's windows ([`window`]) and their UI Automation trees ([`automation`],
//! [`elements`]). Each call that reads or uses a tree runs on a thread of its own that joins a COM apartment for the
//! call and leaves it after, so no apartment of the caller's threads is touched.

mod automation;
mod elements;
mod roles;
mod window;

use std::thread;

use orchester_core::orchester_tools::ui::{UiAction, UiElement, UiRef};

use self::{
  automation::{Automation, Element},
  elements::{Walk, act_on, describe, each, reference},
};
use super::{NativeCapture, NativeSnapshot, NativeUiError, NativeWindow, SNAPSHOT_NODES, crop::crop};

/// The visible top-level windows of the processes `pids`, front first.
pub fn find_windows(pids: &[u32]) -> Result<Vec<NativeWindow>, NativeUiError> {
  window::of_processes(pids).map_err(NativeUiError::Windows)
}

/// `windows` placed off-screen without being activated, as they are then.
pub fn open(windows: &[NativeWindow]) -> Result<Vec<NativeWindow>, NativeUiError> {
  windows
    .iter()
    .map(|window| window::place_off_screen(window).map_err(NativeUiError::Windows))
    .collect()
}

/// The tree of `windows` — the one window, or an `application` holding each — or, with `query`, the elements whose
/// name, automation id or class holds it, any case.
pub fn snapshot(windows: &[NativeWindow], query: Option<&str>) -> Result<NativeSnapshot, NativeUiError> {
  let windows = windows.to_vec();
  let query = query.map(str::to_lowercase);
  on_own_thread(move |automation| {
    let roots = roots(automation, &windows)?;
    match query {
      Some(query) => matches(automation, roots, &query),
      None => Ok(tree(automation, &roots)),
    }
  })
}

fn tree(automation: &Automation, roots: &[Element]) -> NativeSnapshot {
  let mut walk = Walk {
    automation,
    nodes: 0,
    truncated: false,
  };
  let mut trees: Vec<UiElement> = roots.iter().filter_map(|root| walk.tree(root, 0)).collect();
  let tree = if trees.len() == 1 {
    trees.remove(0)
  } else {
    UiElement {
      reference: "app".to_owned(),
      role: "application".to_owned(),
      name: None,
      states: Vec::new(),
      value: None,
      children: trees,
    }
  };
  NativeSnapshot::Tree {
    tree,
    truncated: walk.truncated,
  }
}

fn matches(automation: &Automation, roots: Vec<Element>, query: &str) -> Result<NativeSnapshot, NativeUiError> {
  let mut matches = Vec::new();
  let truncated = each(automation, roots, |element| {
    // An element that went away while it was read matches nothing.
    if let Ok(seen) = describe(element) {
      let texts = [
        seen.element.name.as_deref().unwrap_or(""),
        &seen.automation_id,
        &seen.class,
      ];
      if texts.iter().any(|text| text.to_lowercase().contains(query)) {
        matches.push(seen.element);
      }
    }
    Ok(matches.len() >= SNAPSHOT_NODES)
  })?;
  Ok(NativeSnapshot::Matches { matches, truncated })
}

/// Does `action` to the element `target` of `windows`, through its patterns only.
pub fn act(windows: &[NativeWindow], target: &UiRef, action: &UiAction) -> Result<(), NativeUiError> {
  if matches!(action, UiAction::Key(_)) {
    return Err(NativeUiError::NoKeys);
  }
  let (windows, target, action) = (windows.to_vec(), target.as_str().to_owned(), action.clone());
  on_own_thread(move |automation| {
    let (element, _) = find(automation, &windows, &target)?;
    act_on(automation, (element, &target), &action)
  })
}

/// The first of `windows` as it draws itself, or only the element `target` of the window that holds it.
pub fn capture(windows: &[NativeWindow], target: Option<&UiRef>) -> Result<NativeCapture, NativeUiError> {
  let Some(target) = target else {
    let window = windows.first().ok_or(NativeUiError::NoWindow)?;
    return rendered(window);
  };
  let (owned, target) = (windows.to_vec(), target.as_str().to_owned());
  let (at, rectangle) = on_own_thread(move |automation| {
    let (element, at) = find(automation, &owned, &target)?;
    Ok((at, element.rectangle()?))
  })?;
  let window = windows.get(at).ok_or(NativeUiError::NoWindow)?;
  crop(&rendered(window)?, window, rectangle)
}

/// `window` as it draws itself, in RGBA, opaque.
fn rendered(window: &NativeWindow) -> Result<NativeCapture, NativeUiError> {
  let (width, height, mut pixels) = window::render(window).map_err(NativeUiError::Capture)?;
  for pixel in pixels.chunks_exact_mut(4) {
    pixel.swap(0, 2);
    pixel[3] = u8::MAX;
  }
  Ok(NativeCapture {
    width,
    height,
    rgba: pixels,
  })
}

/// Waits up to `timeout_ms` until every one of `windows` handled what it was sent before.
pub fn idle(windows: &[NativeWindow], timeout_ms: u32) -> Result<(), NativeUiError> {
  if windows.is_empty() {
    return Err(NativeUiError::NoWindow);
  }
  if windows.iter().all(|window| window::settled(window, timeout_ms)) {
    Ok(())
  } else {
    Err(NativeUiError::Busy(timeout_ms))
  }
}

/// The element of each of `windows`.
fn roots(automation: &Automation, windows: &[NativeWindow]) -> Result<Vec<Element>, NativeUiError> {
  if windows.is_empty() {
    return Err(NativeUiError::NoWindow);
  }
  windows
    .iter()
    .map(|window| automation.window(window.handle).map_err(NativeUiError::from))
    .collect()
}

/// The element whose ref is `target`, and which of `windows` holds it.
fn find(automation: &Automation, windows: &[NativeWindow], target: &str) -> Result<(Element, usize), NativeUiError> {
  for (at, root) in roots(automation, windows)?.into_iter().enumerate() {
    let mut found = None;
    each(automation, vec![root], |element| {
      let hit = element.runtime_id().is_ok_and(|ids| reference(&ids) == target);
      if hit {
        found = Some(element.clone());
      }
      Ok(hit)
    })?;
    if let Some(element) = found {
      return Ok((element, at));
    }
  }
  Err(NativeUiError::NoElement(target.to_owned()))
}

/// Runs `work` with UI Automation on a thread of its own.
fn on_own_thread<T: Send>(
  work: impl FnOnce(&Automation) -> Result<T, NativeUiError> + Send,
) -> Result<T, NativeUiError> {
  thread::scope(|scope| {
    scope
      .spawn(|| work(&Automation::new()?))
      .join()
      .map_err(|_| NativeUiError::Automation("the UI Automation thread panicked".to_owned()))?
  })
}
