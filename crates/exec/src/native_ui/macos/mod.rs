//! The native driver on macOS: the run's windows as the window server lists them ([`window`]), each matched to its
//! Accessibility window by its window id, their AX trees ([`ax`], [`tree`]) and their captures through
//! ScreenCaptureKit ([`capture`]). Every call first needs Accessibility and Screen Recording, which macOS grants the
//! responsible app — Orchester, which started the runner — and answers which are missing and where to grant them.
//! Nothing here sends CGEvent input or captures the screen.

mod ax;
mod capture;
mod cf;
mod tree;
mod window;

use orchester_core::orchester_tools::ui::{UiAction, UiElement, UiRef};

use self::{
  ax::AxElement,
  tree::{Walk, act_on, facts, first, window_element},
};
use super::{
  Bounds, NativeCapture, NativePermissions, NativeSnapshot, NativeUiError, NativeWindow, SNAPSHOT_NODES, accessibility,
  crop::crop, ready,
};

/// How far beyond the right of the displays a window is placed, in points.
const OFF_SCREEN_MARGIN: i32 = 64;

/// Which of the driver's permissions macOS grants Orchester now.
pub fn permissions() -> Option<NativePermissions> {
  Some(NativePermissions {
    accessibility: ax::trusted(),
    screen_recording: window::screen_recording(),
  })
}

/// The normal-layer windows of the processes `pids`, front first, each with its Accessibility title.
pub fn find_windows(pids: &[u32]) -> Result<Vec<NativeWindow>, NativeUiError> {
  ready()?;
  let displays = window::displays();
  let listed = window::of_processes(pids).map_err(NativeUiError::Windows)?;
  Ok(
    listed
      .into_iter()
      .map(|listed| {
        let mut found = NativeWindow {
          window_id: listed.window_id,
          pid: listed.pid,
          title: listed.name.unwrap_or_default(),
          bounds: listed.bounds,
          off_screen: !shown(listed.bounds, &displays),
        };
        // A window AX does not reach keeps the window server's title; a snapshot of it answers why it is not reached.
        let title = window_element(&found)
          .ok()
          .and_then(|element| element.text("AXTitle").ok().flatten());
        if let Some(title) = title {
          found.title = title;
        }
        found
      })
      .collect(),
  )
}

/// Whether any display shows part of `bounds`.
fn shown(bounds: Bounds, displays: &[Bounds]) -> bool {
  displays.iter().any(|display| {
    bounds.x < display.x + display.width
      && display.x < bounds.x + bounds.width
      && bounds.y < display.y + display.height
      && display.y < bounds.y + bounds.height
  })
}

/// `windows` moved beyond the right of every display through AX, without being activated, as they are then. The app
/// may keep a window nearer the displays; `off_screen` says where it is.
pub fn open(windows: &[NativeWindow]) -> Result<Vec<NativeWindow>, NativeUiError> {
  ready()?;
  let displays = window::displays();
  let right = displays
    .iter()
    .map(|display| display.x + display.width)
    .max()
    .unwrap_or(0);
  let top = displays.iter().map(|display| display.y).min().unwrap_or(0);
  windows
    .iter()
    .map(|placed| {
      if placed.off_screen {
        return Ok(placed.clone());
      }
      window_element(placed)?.move_to(f64::from(right + OFF_SCREEN_MARGIN), f64::from(top))?;
      let moved = window::of_processes(&[placed.pid])
        .map_err(NativeUiError::Windows)?
        .into_iter()
        .find(|listed| listed.window_id == placed.window_id)
        .ok_or(NativeUiError::NoWindow)?;
      Ok(NativeWindow {
        bounds: moved.bounds,
        off_screen: !shown(moved.bounds, &displays),
        ..placed.clone()
      })
    })
    .collect()
}

/// The tree of `windows` — the one window, or an `application` holding each — or, with `query`, the elements whose
/// name, identifier or AX role holds it, any case.
pub fn snapshot(windows: &[NativeWindow], query: Option<&str>) -> Result<NativeSnapshot, NativeUiError> {
  ready()?;
  let roots = roots(windows)?;
  let Some(query) = query.map(str::to_lowercase) else {
    let mut walk = Walk::default();
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
    return Ok(NativeSnapshot::Tree {
      tree,
      truncated: walk.truncated,
    });
  };
  let mut matches = Vec::new();
  let full = first(roots, |element| {
    // An element that went away while it was read matches nothing.
    if let Ok(seen) = facts(element)
      && seen.matches(&query)
    {
      matches.push(seen.element());
    }
    Ok(matches.len() >= SNAPSHOT_NODES)
  })?;
  Ok(NativeSnapshot::Matches {
    matches,
    truncated: full.is_some(),
  })
}

/// Does `action` to the element `target` of `windows`, through its AX actions and attributes only.
pub fn act(windows: &[NativeWindow], target: &UiRef, action: &UiAction) -> Result<(), NativeUiError> {
  ready()?;
  if matches!(action, UiAction::Key(_)) {
    return Err(NativeUiError::NoKeys);
  }
  let (element, _) = find(windows, target.as_str())?;
  act_on(&element, action)
}

/// The first of `windows` alone, by its window id, or only the element `target` of the window that holds it.
pub fn capture(windows: &[NativeWindow], target: Option<&UiRef>) -> Result<NativeCapture, NativeUiError> {
  ready()?;
  let Some(target) = target else {
    let window = windows.first().ok_or(NativeUiError::NoWindow)?;
    return capture::window_image(window).map_err(NativeUiError::Capture);
  };
  let (element, at) = find(windows, target.as_str())?;
  let frame = element
    .frame()?
    .ok_or_else(|| NativeUiError::Capture(format!("the element `{}` tells no position and size", target.as_str())))?;
  let window = windows.get(at).ok_or(NativeUiError::NoWindow)?;
  let whole = capture::window_image(window).map_err(NativeUiError::Capture)?;
  let bounds = window::bounds_of(frame);
  crop(
    &whole,
    window,
    (bounds.x, bounds.y, bounds.x + bounds.width, bounds.y + bounds.height),
  )
}

/// Waits up to `timeout_ms` until the app of every one of `windows` answers Accessibility: what its main thread was
/// busy with before is done.
pub fn idle(windows: &[NativeWindow], timeout_ms: u32) -> Result<(), NativeUiError> {
  ready()?;
  if windows.is_empty() {
    return Err(NativeUiError::NoWindow);
  }
  // Seconds as AX takes them; a timeout of milliseconds fits an f32 closely enough.
  let seconds = (f64::from(timeout_ms) / 1000.0) as f32;
  for window in windows {
    let element = window_element(window)?;
    element.answer_within(seconds)?;
    match element.text("AXRole") {
      Ok(_) => {}
      Err(failed) if failed.busy() => return Err(NativeUiError::Busy(timeout_ms)),
      Err(failed) => return Err(failed.into()),
    }
  }
  Ok(())
}

/// The AX window of each of `windows`.
fn roots(windows: &[NativeWindow]) -> Result<Vec<AxElement>, NativeUiError> {
  if windows.is_empty() {
    return Err(NativeUiError::NoWindow);
  }
  windows.iter().map(window_element).collect()
}

/// The element whose ref is `target`, and which of `windows` holds it.
fn find(windows: &[NativeWindow], target: &str) -> Result<(AxElement, usize), NativeUiError> {
  for (at, root) in roots(windows)?.into_iter().enumerate() {
    let found = first(vec![root], |element| {
      Ok(accessibility::reference(element.hash()) == target)
    })?;
    if let Some(element) = found {
      return Ok((element, at));
    }
  }
  Err(NativeUiError::NoElement(target.to_owned()))
}
