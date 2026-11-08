//! How `orchester-fake-codex` drives the browser driver's fixture page (`fake_server.rs`, `/app`) through the `ui_*`
//! tools, for `tests/browser_ui.rs`: "Then drive the web page" opens the service, waits for its address, finds the
//! page's elements by the snapshot's roles and names, looks for Save by a query, types a title, chooses a priority,
//! presses Tab in the field, clicks Save, waits for the text, a selector and idle, takes a screenshot of the page and
//! one of Save, reads the log, clicks the link to another origin and reads the log until it reports the blocked page,
//! then acts on a ref of no element.

use std::{thread, time::Duration};

use serde_json::{Value, json};

/// How often the log is read again while the blocked page is not reported yet.
const LOOKS: usize = 40;

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

/// "Then drive the web page" of the service `name`, each call made through `call`, which reports it and answers its
/// text.
pub fn drive(name: &str, call: &mut dyn FnMut(&str, &Value) -> Option<String>) {
  call("ui_open", &json!({"service": name}));
  call("ui_wait", &json!({"condition": "url", "query": "/app"}));
  let snapshot = call("ui_snapshot", &json!({}))
    .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    .unwrap_or_default();
  let tree = &snapshot["tree"];
  let (Some(title), Some(priority), Some(save), Some(away)) = (
    find(tree, "textbox", "Title"),
    find(tree, "combobox", "Priority"),
    find(tree, "button", "Save"),
    find(tree, "link", "Elsewhere"),
  ) else {
    eprintln!("orchester-web: the snapshot lacks the fixture's elements: {snapshot}");
    return;
  };
  call("ui_snapshot", &json!({"query": "save"}));
  call("ui_act", &json!({"ref": title, "action": "type", "text": "Fix login"}));
  call("ui_act", &json!({"ref": priority, "action": "select", "text": "High"}));
  call("ui_act", &json!({"ref": title, "action": "key", "text": "Tab"}));
  call("ui_act", &json!({"ref": save, "action": "click"}));
  call(
    "ui_wait",
    &json!({"condition": "text", "query": "Saved: Fix login (High)"}),
  );
  call("ui_wait", &json!({"condition": "selector", "query": "#status.saved"}));
  call("ui_wait", &json!({"condition": "idle"}));
  call("ui_screenshot", &json!({}));
  call("ui_screenshot", &json!({"ref": save}));
  call("ui_logs", &json!({"lines": 50}));
  call("ui_act", &json!({"ref": away, "action": "click"}));
  for _ in 0..LOOKS {
    let logged = call("ui_logs", &json!({"lines": 50, "filter": "blocked"})).unwrap_or_default();
    if logged.contains("example.com/away") {
      break;
    }
    thread::sleep(Duration::from_millis(250));
  }
  call("ui_act", &json!({"ref": "b999999", "action": "click"}));
}
