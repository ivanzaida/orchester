//! The `native` UI driver for a project's service (`orchester_exec::native_ui`): each call finds the service's windows
//! again — the visible top-level windows of its program's tree whose processes pass the check `stop_run_process` makes
//! ([`RunProcesses::tree`]) — and sees or uses them through UI Automation on Windows, Accessibility on macOS. `ui_open`
//! places them off-screen without activating them; a screenshot is that window alone, saved in the run's storage as an
//! app's is; the log is the service's own output.

use std::{
  num::NonZeroU32,
  path::Path,
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  orchester_tools::{
    UiCall,
    ui::{UiCondition, mcp::WAIT_POLL_MS},
  },
  service::ServiceName,
};
use orchester_exec::{
  native_ui::{self, NativeSnapshot, NativeUiError, NativeWindow},
  run_processes::RunProcesses,
};
use serde_json::{Value, json};

use super::super::{run_app::save_pixels, service::StepServices};

/// The service tools and processes the native driver reaches a service through.
pub(super) struct NativeUi<'a> {
  pub(super) processes: &'a RunProcesses,
  pub(super) services: &'a StepServices,
  /// The run's own storage, where screenshots go.
  pub(super) artifacts: &'a Path,
}

fn failed(error: &NativeUiError) -> String {
  error.to_string()
}

impl NativeUi<'_> {
  /// `ui_open(name)`: its windows, placed off-screen without being activated.
  pub(super) fn open(&self, name: &ServiceName) -> Result<Value, String> {
    let (pid, windows) = self.windows(name)?;
    let placed = native_ui::open(&windows).map_err(|error| failed(&error))?;
    Ok(json!({
      "service": name.as_str(),
      "driver": "native",
      "pid": pid,
      "windows": placed.iter().map(window).collect::<Vec<_>>(),
    }))
  }

  /// A call on the service `name`, which `ui_open` opened.
  pub(super) fn run(&self, name: &ServiceName, call: &UiCall) -> Result<Value, String> {
    match call {
      UiCall::Open(_) => self.open(name),
      UiCall::Snapshot { query } => self.snapshot(name, query.as_deref()),
      UiCall::Act { target, action } => {
        let Some(target) = target else {
          return Err(NativeUiError::NoKeys.to_string());
        };
        let (_, windows) = self.windows(name)?;
        native_ui::act(&windows, target, action).map_err(|error| failed(&error))?;
        Ok(json!({"acted": action.as_str(), "ref": target.as_str()}))
      }
      UiCall::Screenshot { target } => {
        let (_, windows) = self.windows(name)?;
        let capture = native_ui::capture(&windows, target.as_ref()).map_err(|error| failed(&error))?;
        let saved = save_pixels(self.artifacts, (capture.width, capture.height, capture.rgba))?;
        Ok(json!({"screenshot": saved}))
      }
      UiCall::Logs { lines, filter } => {
        let lines = self
          .services
          .log_lines(name, *lines, filter.as_deref())
          .ok_or_else(|| format!("the service `{name}` was not started in this step"))?;
        Ok(json!({"lines": lines}))
      }
      UiCall::Wait {
        condition: UiCondition::Idle,
        timeout_ms,
      } => {
        let (_, windows) = self.windows(name)?;
        native_ui::idle(&windows, *timeout_ms).map_err(|error| failed(&error))?;
        Ok(json!({"met": "idle"}))
      }
      UiCall::Wait {
        condition: UiCondition::Element(query),
        timeout_ms,
      } => self.wait_for(name, query, *timeout_ms),
    }
  }

  fn snapshot(&self, name: &ServiceName, query: Option<&str>) -> Result<Value, String> {
    let (_, windows) = self.windows(name)?;
    Ok(
      match native_ui::snapshot(&windows, query).map_err(|error| failed(&error))? {
        NativeSnapshot::Tree { tree, truncated } => json!({"tree": tree, "truncated": truncated}),
        NativeSnapshot::Matches { matches, truncated } => json!({"matches": matches, "truncated": truncated}),
      },
    )
  }

  /// Looks at the service's windows until an element `query` names is shown, at most `timeout_ms`.
  fn wait_for(&self, name: &ServiceName, query: &str, timeout_ms: u32) -> Result<Value, String> {
    let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_ms));
    loop {
      let answer = self.snapshot(name, Some(query))?;
      if answer["matches"].as_array().is_some_and(|matches| !matches.is_empty()) {
        return Ok(json!({"met": "element", "matches": answer["matches"]}));
      }
      if Instant::now() >= deadline {
        return Err(format!("no element holding `{query}` was shown within {timeout_ms} ms"));
      }
      thread::sleep(Duration::from_millis(WAIT_POLL_MS));
    }
  }

  /// The pid of the service's program and its windows, of the processes of its tree this run may reach; why there are
  /// none.
  fn windows(&self, name: &ServiceName) -> Result<(u32, Vec<NativeWindow>), String> {
    let pid = self.services.pid(name).ok_or_else(|| {
      format!("the service `{name}` is not running in this step: start it with start_service(\"{name}\") first")
    })?;
    let root = NonZeroU32::new(pid).ok_or_else(|| format!("the service `{name}` has no process"))?;
    let tree = self
      .processes
      .tree(root)
      .map_err(|error| format!("the processes of the service `{name}` cannot be read: {error}"))?;
    let reached = tree.pids();
    let windows = if reached.is_empty() {
      Vec::new()
    } else {
      native_ui::find_windows(&reached).map_err(|error| failed(&error))?
    };
    if !windows.is_empty() {
      return Ok((pid, windows));
    }
    if tree.refused().is_empty() {
      return Err(format!(
        "the service `{name}` shows no window yet: wait for it with ui_wait(\"element\", query) or call again"
      ));
    }
    Err(format!(
      "refused: no window of the service `{name}` belongs to a process this run may reach; {}",
      tree.refused().join("; ")
    ))
  }
}

/// A window as `ui_open` answers it.
fn window(window: &NativeWindow) -> Value {
  let bounds = window.bounds;
  json!({
    "title": window.title,
    "pid": window.pid,
    "bounds": {"x": bounds.x, "y": bounds.y, "width": bounds.width, "height": bounds.height},
    "off_screen": window.off_screen,
  })
}
