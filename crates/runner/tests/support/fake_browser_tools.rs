//! How the fake CLIs use the browser tools ("Use the browser tools"), on the pages the test serves at the address it
//! writes into `fake-browser-site` beside the worktrees; `fake-browser-other-tab` names a tab another run opened. With
//! `fake-browser-elsewhere`, the same pages at an address whose host is not local, the fake reads and acts as
//! [`read_closely_and_act_elsewhere`] says instead. Each answer is reported
//! (`browser-call: <tool> <ok|error>[ +image]: <first line>`), and each of its lines (`browser-answer: <tool> |
//! <line>`). Without a site the fake only lists the run's tabs: the step may not have the browser.

use std::{fs, path::Path};

use serde_json::{Value, json};

/// A call of a tool of Orchester's server: its whole MCP result, `None` when the server did not answer.
type Call<'a> = &'a mut dyn FnMut(&str, &Value) -> Option<Value>;

pub fn exercise(worktree: &Path, call: Call<'_>) {
  let Some(site) = written(worktree, "fake-browser-site") else {
    report(call, "browser_tabs", &json!({}));
    return;
  };
  if let Some(elsewhere) = written(worktree, "fake-browser-elsewhere") {
    read_closely_and_act_elsewhere(&site, &elsewhere, call);
    return;
  }
  report(call, "browser_tabs", &json!({}));
  if let Some(other) = written(worktree, "fake-browser-other-tab") {
    report(call, "browser_read_page", &json!({"tab": other}));
  }
  report(call, "browser_click", &json!({"ref": "e1"}));
  report(call, "browser_open", &json!({"url": format!("{site}form")}));
  let page = report(call, "browser_read_page", &json!({}));
  let (field, save) = (element(&page, "textbox \"Name\""), element(&page, "button \"Save\""));
  report(call, "browser_type", &json!({"ref": field, "text": "Ada"}));
  report(call, "browser_click", &json!({"ref": save}));
  report(call, "browser_wait", &json!({"text": "Saved Ada", "timeout": 5}));
  report(call, "browser_page_text", &json!({}));
  let fresh = report(call, "browser_read_page", &json!({"interactive_only": true}));
  report(call, "browser_click", &json!({"ref": save}));
  report(call, "browser_key", &json!({"keys": ["Tab"]}));
  report(call, "browser_scroll", &json!({"direction": "down", "pixels": 300}));
  report(call, "browser_screenshot", &json!({}));
  let button = element(&fresh, "button \"Save\"");
  report(call, "browser_screenshot", &json!({"ref": button}));
  report(call, "browser_screenshot", &json!({"full_page": true}));
  report(call, "browser_scroll", &json!({"ref": button}));
  report(call, "browser_wait", &json!({"ref": button, "timeout": 2}));
  report(call, "browser_console", &json!({}));
  report(call, "browser_console", &json!({"errors_only": true}));
  report(call, "browser_network", &json!({"url_pattern": "/api/"}));
  report(call, "browser_open", &json!({"url": format!("{site}later")}));
  report(call, "browser_navigate", &json!({"to": "back"}));
  report(
    call,
    "browser_wait",
    &json!({"text": "never on the page", "timeout": 1}),
  );
  report(call, "browser_eval", &json!({"expression": "document.title"}));
  report(
    call,
    "browser_open",
    &json!({"url": "http://docs.example.invalid/", "new_tab": true}),
  );
  report(
    call,
    "browser_open",
    &json!({"url": "http://elsewhere.example.invalid/"}),
  );
  report(call, "browser_close", &json!({}));
  report(call, "browser_tabs", &json!({}));
}

/// Reads the form filtered by a query, cut at `max_chars` and as its interactive elements only, and its text cut short;
/// saves, reloads, goes to a later page, back and forward again; dismisses a `confirm()` that then throws and reads the
/// console's errors. Then, on `elsewhere` (a host neither local nor allowed), opens the form, clicks Save twice and
/// types into its field: the test answers the first click with Approve similar.
fn read_closely_and_act_elsewhere(site: &str, elsewhere: &str, call: Call<'_>) {
  report(call, "browser_open", &json!({"url": format!("{site}form")}));
  report(call, "browser_read_page", &json!({"query": "SAVE"}));
  report(call, "browser_read_page", &json!({"max_chars": 30}));
  report(call, "browser_page_text", &json!({"max_chars": 10}));
  let form = report(call, "browser_read_page", &json!({"interactive_only": true}));
  let (field, save) = (element(&form, "textbox \"Name\""), element(&form, "button \"Save\""));
  report(call, "browser_type", &json!({"ref": field, "text": "Ada"}));
  report(call, "browser_click", &json!({"ref": save}));
  report(call, "browser_navigate", &json!({"to": "reload"}));
  report(call, "browser_page_text", &json!({}));
  report(call, "browser_open", &json!({"url": format!("{site}later")}));
  report(call, "browser_navigate", &json!({"to": "back"}));
  report(call, "browser_navigate", &json!({"to": "forward"}));
  report(call, "browser_open", &json!({"url": format!("{site}confirm")}));
  let page = report(call, "browser_read_page", &json!({"interactive_only": true}));
  report(
    call,
    "browser_click",
    &json!({"ref": element(&page, "button \"Delete\"")}),
  );
  report(call, "browser_page_text", &json!({}));
  report(call, "browser_console", &json!({"errors_only": true}));
  report(call, "browser_open", &json!({"url": format!("{elsewhere}form")}));
  let form = report(call, "browser_read_page", &json!({"interactive_only": true}));
  let (field, save) = (element(&form, "textbox \"Name\""), element(&form, "button \"Save\""));
  report(call, "browser_click", &json!({"ref": save}));
  report(call, "browser_click", &json!({"ref": save}));
  report(call, "browser_type", &json!({"ref": field, "text": "Bo"}));
}

/// What the test wrote into `name` beside the worktrees.
fn written(worktree: &Path, name: &str) -> Option<String> {
  let folder = worktree.ancestors().nth(2)?;
  let text = fs::read_to_string(folder.join(name)).ok()?;
  Some(text.trim().to_owned())
}

/// The ref of the element whose line in `page` holds `named`.
fn element(page: &str, named: &str) -> String {
  page
    .lines()
    .find(|line| line.contains(named))
    .and_then(|line| line.rsplit_once('[')?.1.strip_suffix(']').map(str::to_owned))
    .unwrap_or_default()
}

/// Calls `tool` with `arguments` and reports its answer; its text.
fn report(call: Call<'_>, tool: &str, arguments: &Value) -> String {
  let Some(result) = call(tool, arguments) else {
    eprintln!("browser-call: {tool} no answer");
    return String::new();
  };
  let blocks = result["content"].as_array().cloned().unwrap_or_default();
  let text: Vec<&str> = blocks.iter().filter_map(|block| block["text"].as_str()).collect();
  let text = text.join("\n");
  let image = blocks
    .iter()
    .any(|block| block["type"] == "image" && block["data"].as_str().is_some_and(|data| !data.is_empty()));
  let status = if result["isError"] == true { "error" } else { "ok" };
  let shown = if image { " +image" } else { "" };
  eprintln!(
    "browser-call: {tool} {status}{shown}: {}",
    text.lines().next().unwrap_or_default()
  );
  for line in text.lines() {
    eprintln!("browser-answer: {tool} | {line}");
  }
  text
}
