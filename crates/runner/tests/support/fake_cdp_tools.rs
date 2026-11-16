//! How `orchester-fake-codex` drives an Electron-style app's window through the `ui_*` tools, for the `cdp` driver's
//! tests (`tests/cdp_ui.rs`): "Copy the launcher fixture." puts `orchester-test-command` into the worktree first, where
//! a service of the run may start it as the app's launcher; "Then drive the app window" reads the service's log, opens
//! it, waits for its address and until it is idle, finds its field and Save by the snapshot's roles and names, types a
//! title, clicks Save, waits for the status's text and for the element showing it, takes a screenshot and reads the
//! console; "Then open the UI of <name>." only opens that service.

use std::path::Path;

use serde_json::{Value, json};

/// Copies `orchester-test-command`, as cargo built it beside this program, to `fixture/` in `worktree`.
pub fn copy_launcher(worktree: &Path) {
  let copied = std::env::current_exe().ok().and_then(|program| {
    let launcher = program
      .parent()?
      .join(format!("orchester-test-command{}", std::env::consts::EXE_SUFFIX));
    let folder = worktree.join("fixture");
    std::fs::create_dir_all(&folder).ok()?;
    std::fs::copy(&launcher, folder.join(launcher.file_name()?)).ok()
  });
  if copied.is_none() {
    eprintln!("cannot copy the launcher fixture into the worktree");
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

/// "Then drive the app window" of the service `name`, each call made through `call`, which reports it and answers its
/// text.
pub fn drive(name: &str, call: &mut dyn FnMut(&str, &Value) -> Option<String>) {
  call("service_logs", &json!({"name": name, "tail": 200}));
  call("ui_open", &json!({"service": name}));
  call("ui_wait", &json!({"condition": "url", "query": "window.html"}));
  call("ui_wait", &json!({"condition": "idle"}));
  let snapshot = call("ui_snapshot", &json!({}))
    .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    .unwrap_or_default();
  let tree = &snapshot["tree"];
  let (Some(title), Some(save)) = (find(tree, "textbox", "Title"), find(tree, "button", "Save")) else {
    eprintln!("orchester-cdp: the snapshot lacks the fixture's elements: {snapshot}");
    return;
  };
  call("ui_act", &json!({"ref": title, "action": "type", "text": "Fix login"}));
  call("ui_act", &json!({"ref": save, "action": "click"}));
  call("ui_wait", &json!({"condition": "text", "query": "Saved: Fix login"}));
  call(
    "ui_wait",
    &json!({"condition": "element", "query": "Saved: Fix login", "timeout_ms": 10000}),
  );
  call("ui_screenshot", &json!({}));
  call("ui_logs", &json!({"lines": 20}));
}
