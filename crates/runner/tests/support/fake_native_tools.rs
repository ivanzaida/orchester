//! How `orchester-fake-codex` drives the native fixture window (`orchester-test-window`) through the `ui_*` tools, for
//! the native driver's tests (`tests/native_ui.rs`): "Copy the native fixture." puts the fixture program into the
//! worktree first, where a service of the run may start it; "Then drive the native window" opens it, finds its
//! elements by the snapshot's roles and names, types into its field, clicks Save, waits for the status to say so,
//! tries a click on the label and a key, which have no pattern, ticks the checkbox, looks again, takes a screenshot,
//! reads the log and waits until it is idle; "Then open its UI" only opens it.

use std::path::Path;

use serde_json::{Value, json};

/// The fixture program as cargo built it beside this one.
fn built_fixture() -> Option<std::path::PathBuf> {
  let program = std::env::current_exe().ok()?;
  Some(
    program
      .parent()?
      .join(format!("orchester-test-window{}", std::env::consts::EXE_SUFFIX)),
  )
}

/// Copies the fixture program to `fixture/` in `worktree`.
pub fn copy_fixture(worktree: &Path) {
  let copied = built_fixture().and_then(|fixture| {
    let folder = worktree.join("fixture");
    std::fs::create_dir_all(&folder).ok()?;
    std::fs::copy(&fixture, folder.join(fixture.file_name()?)).ok()
  });
  if copied.is_none() {
    eprintln!("cannot copy the native fixture into the worktree");
  }
}

/// The ref of the first element of `tree` with `role` whose name starts with `name`.
fn find(tree: &Value, role: &str, name: &str) -> Option<String> {
  let starts = tree["name"].as_str().is_some_and(|own| own.starts_with(name));
  if tree["role"] == role && starts {
    return tree["ref"].as_str().map(str::to_owned);
  }
  tree["children"]
    .as_array()
    .into_iter()
    .flatten()
    .find_map(|child| find(child, role, name))
}

/// "Then drive the native window" of the service `name`, each call made through `call`, which reports it and answers
/// its text.
pub fn drive(name: &str, call: &mut dyn FnMut(&str, &Value) -> Option<String>) {
  call("ui_open", &json!({"service": name}));
  let snapshot = call("ui_snapshot", &json!({}))
    .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    .unwrap_or_default();
  let tree = &snapshot["tree"];
  let (Some(field), Some(save), Some(status), Some(remember)) = (
    find(tree, "edit", ""),
    find(tree, "button", "Save"),
    find(tree, "text", "Status"),
    find(tree, "checkbox", "Remember"),
  ) else {
    eprintln!("orchester-native: the snapshot lacks the fixture's elements");
    return;
  };
  eprintln!("orchester-native: status {status}");
  call("ui_act", &json!({"ref": field, "action": "type", "text": "Fix login"}));
  call("ui_act", &json!({"ref": save, "action": "click"}));
  call(
    "ui_wait",
    &json!({"condition": "element", "query": "Saved: Fix login", "timeout_ms": 10000}),
  );
  call("ui_act", &json!({"ref": status, "action": "click"}));
  call("ui_act", &json!({"action": "key", "text": "Enter"}));
  call("ui_act", &json!({"ref": remember, "action": "click"}));
  call("ui_snapshot", &json!({}));
  call("ui_screenshot", &json!({}));
  call("ui_logs", &json!({"lines": 20}));
  call("ui_wait", &json!({"condition": "idle", "timeout_ms": 5000}));
}
