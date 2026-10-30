//! How the fake CLIs use the browser tools ("Use the browser tools"), on the pages the test serves at the address it
//! writes into `fake-browser-site` beside the worktrees; `fake-browser-other-tab` names a tab another run opened. With
//! `fake-browser-elsewhere`, the same pages at an address whose host is not local, the fake reads and acts as
//! [`read_closely_and_act_elsewhere`] says instead. With `fake-browser-hold`, it acts on a tab the test uses as the
//! person, step by step as the test says ([`act_on_a_held_tab`]). Each answer is reported
//! (`browser-call: <tool> <ok|error>[ +image]: <first line>`), and each of its lines (`browser-answer: <tool> |
//! <line>`). Without a site the fake only lists the run's tabs: the step may not have the browser.

use std::{fs, path::Path, thread, time::Duration};

use serde_json::{Value, json};

/// A call of a tool of Orchester's server: its whole MCP result, `None` when the server did not answer.
type Call<'a> = &'a mut dyn FnMut(&str, &Value) -> Option<Value>;

/// How long the fake waits for the test's word, and how often it looks.
const GO_AHEAD_POLLS: u32 = 2400;
const GO_AHEAD_POLL: Duration = Duration::from_millis(50);

pub fn exercise(worktree: &Path, call: Call<'_>) {
  let Some(site) = written(worktree, "fake-browser-site") else {
    report(call, "browser_tabs", &json!({}));
    return;
  };
  if let Some(elsewhere) = written(worktree, "fake-browser-elsewhere") {
    read_closely_and_act_elsewhere(&site, &elsewhere, call);
    return;
  }
  if let Some(script) = written(worktree, "fake-browser-hold") {
    act_on_a_held_tab(worktree, &site, &script, call);
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

/// Acts on a tab the test uses as the person, each step when the test says so (`fake-browser-go-<step>`). `script` is
/// `every-call`: all the reading tools and then one call of each that waits; `site-moves`: a key press and scripts on a
/// tab the person sends elsewhere; or `one-click`: a click, the page's text, and both again.
fn act_on_a_held_tab(worktree: &Path, site: &str, script: &str, call: Call<'_>) {
  report(call, "browser_open", &json!({"url": format!("{site}form")}));
  if script == "every-call" {
    every_call(worktree, site, call);
    return;
  }
  if script == "site-moves" {
    site_moves(worktree, call);
    return;
  }
  let form = report(call, "browser_read_page", &json!({}));
  let save = element(&form, "button \"Save\"");
  go_ahead(worktree, 0);
  report(call, "browser_click", &json!({"ref": save}));
  report(call, "browser_page_text", &json!({}));
  go_ahead(worktree, 1);
  report(call, "browser_click", &json!({"ref": save}));
  report(call, "browser_page_text", &json!({}));
}

/// A key press, then a script that shows whether it moved the focus, then two more.
fn site_moves(worktree: &Path, call: Call<'_>) {
  go_ahead(worktree, 0);
  report(call, "browser_key", &json!({"keys": ["Tab"]}));
  go_ahead(worktree, 1);
  report(
    call,
    "browser_eval",
    &json!({"expression": "document.activeElement.tagName"}),
  );
  for step in 2..=3 {
    go_ahead(worktree, step);
    report(call, "browser_eval", &json!({"expression": "location.pathname"}));
  }
}

/// Reads every way there is, then makes one call of each tool that waits, the test holding the tab before each. The
/// last closes the tab the test names, as a new tab became the current one before it.
fn every_call(worktree: &Path, site: &str, call: Call<'_>) {
  go_ahead(worktree, 0);
  report(call, "browser_tabs", &json!({}));
  report(call, "browser_read_page", &json!({}));
  report(call, "browser_page_text", &json!({}));
  report(call, "browser_screenshot", &json!({}));
  report(call, "browser_console", &json!({}));
  report(call, "browser_network", &json!({}));
  report(call, "browser_wait", &json!({"text": "Guest book", "timeout": 5}));
  go_ahead(worktree, 1);
  report(call, "browser_open", &json!({"url": format!("{site}later")}));
  go_ahead(worktree, 2);
  report(call, "browser_navigate", &json!({"to": "back"}));
  let fresh = report(call, "browser_read_page", &json!({}));
  go_ahead(worktree, 3);
  let field = element(&fresh, "textbox \"Name\"");
  report(call, "browser_type", &json!({"ref": field, "text": "Ada"}));
  go_ahead(worktree, 4);
  report(
    call,
    "browser_click",
    &json!({"ref": element(&fresh, "button \"Save\"")}),
  );
  go_ahead(worktree, 5);
  report(call, "browser_key", &json!({"keys": ["Tab"]}));
  go_ahead(worktree, 6);
  report(call, "browser_scroll", &json!({"direction": "down", "pixels": 300}));
  go_ahead(worktree, 7);
  report(call, "browser_eval", &json!({"expression": "document.title"}));
  go_ahead(worktree, 8);
  report(
    call,
    "browser_open",
    &json!({"url": format!("{site}other"), "new_tab": true}),
  );
  let first_tab = go_ahead(worktree, 9);
  report(call, "browser_close", &json!({"tab": first_tab}));
}

/// Waits until the test wrote `fake-browser-go-<step>`; what it wrote.
fn go_ahead(worktree: &Path, step: u32) -> String {
  let name = format!("fake-browser-go-{step}");
  for _ in 0..GO_AHEAD_POLLS {
    if let Some(word) = written(worktree, &name) {
      return word;
    }
    thread::sleep(GO_AHEAD_POLL);
  }
  panic!("the test did not say {name} in time");
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
