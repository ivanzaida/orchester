//! What an agent is told about each `ui_*` tool: its description and strict input schema.

use serde_json::{Value, json};

use super::{
  DEFAULT_LOG_LINES, DEFAULT_WAIT_MS, MAX_LOG_LINES, MAX_QUERY_CHARS, MAX_REF_CHARS, MAX_TEXT_CHARS, MAX_WAIT_MS,
};
use crate::orchester_tools::OrchesterTool;

/// What `ui_open` does.
const OPENS: &str = "See and use the UI of a service you started in this step with start_service (desktop-qa is the QA copy of the app); the other ui_* tools reach it from then on. Returns {service, driver, pid, windows}; a web service (browser driver) opens in a tab of this run in Orchester's Browser and returns {service, driver, tab, url, title, allowed_origins}: that tab opens only the origins of the services this step started, and every other navigation or request is blocked and reported; an Electron or WebView2 app (cdp driver) is reached through its remote debugging port, the service's {{port}}, and returns {service, driver, pid, port, window, url, title, windows}. Runs without asking; only apps this run started are reached.";

/// What `ui_snapshot` does.
const SNAPSHOTS: &str = "The UI of the service ui_open opened as a tree of elements: {tree: {ref, role, name, children}, truncated}; with query, only the elements whose name, id or class holds it: {matches: [{ref, role, name}], truncated}. Each snapshot hands out new refs: take a new one after the UI changed. Use it before ui_act, and instead of a screenshot when the names say enough.";

/// What `ui_act` does.
const ACTS: &str = "Act on an element of the latest ui_snapshot by its ref, through the app's own input, never the computer's: click; type (text into it); key (a key, e.g. Enter, Tab, Escape, ArrowDown or a, in the ref's window or the focused element without ref); scroll (into view); select (an option by its label, or a field's value). Returns {acted, ref}; a stale ref fails: take a new snapshot.";

/// What `ui_screenshot` does.
const SCREENSHOTS: &str = "A PNG of the opened service's window, or only of the element ref names, saved in the run's own storage (.orchester/runs/<run>/screenshots/, listed on its task page under Artifacts): returns {screenshot: <absolute path>}; open it with read_file to see it. Only the app's window is captured, never the screen.";

/// What `ui_logs` does.
const LOGS: &str = "The latest log lines of the opened service's app, newest last, only those holding filter when it is given: {lines: [...]}. For a web service: its page's console errors, its failed requests (4xx, 5xx or not answered) and what was blocked.";

/// What `ui_wait` does.
const WAITS: &str = "Wait until the opened service's UI holds condition: idle (it rendered and has nothing more to draw; use it before a screenshot) or element (an element whose name, id or class holds query is shown); for a web service also selector (a CSS selector matches an element), text (the page's text holds query) or url (the page's address holds query). Returns {met, matches} once it holds; an error when timeout_ms passed first.";

/// What the `ui_*` tool `tool` does; empty for any other tool, which says it itself (`OrchesterTool::description`).
pub(in crate::orchester_tools) const fn description(tool: OrchesterTool) -> &'static str {
  match tool {
    OrchesterTool::UiOpen => OPENS,
    OrchesterTool::UiSnapshot => SNAPSHOTS,
    OrchesterTool::UiAct => ACTS,
    OrchesterTool::UiScreenshot => SCREENSHOTS,
    OrchesterTool::UiLogs => LOGS,
    OrchesterTool::UiWait => WAITS,
    _ => "",
  }
}

/// The input schema of the `ui_*` tool `tool`, `services` being the services `ui_open` takes; one taking nothing for
/// any other tool, which has a schema of its own (`super::super::catalog`).
pub(in crate::orchester_tools) fn input_schema(tool: OrchesterTool, services: &[String]) -> Value {
  let reference =
    |description: &str| json!({"type":["string","null"],"maxLength":MAX_REF_CHARS,"description":description});
  let query = |description: &str| json!({"type":["string","null"],"minLength":1,"maxLength":MAX_QUERY_CHARS,"description":description});
  match tool {
    OrchesterTool::UiOpen => json!({"type":"object","properties":{
      "service":{"type":"string","enum":services,"description":"The service whose UI to open, started in this step."}
    },"required":["service"],"additionalProperties":false}),
    OrchesterTool::UiSnapshot => json!({"type":"object","properties":{
      "query":query("Only the elements whose name, id or class holds this, any case; null for the whole tree.")
    },"additionalProperties":false}),
    OrchesterTool::UiAct => json!({"type":"object","properties":{
      "ref":reference("The element's ref from the latest ui_snapshot; null only for key, which then goes to the focused element."),
      "action":{"type":"string","enum":["click","type","key","scroll","select"],"description":"What to do to the element."},
      "text":{"type":["string","null"],"minLength":1,"maxLength":MAX_TEXT_CHARS,
        "description":"type: the text to type; key: the key's name; select: the option's label or the field's value; null for click and scroll."}
    },"required":["action"],"additionalProperties":false}),
    OrchesterTool::UiScreenshot => json!({"type":"object","properties":{
      "ref":reference("Only this element of the latest ui_snapshot; null for the whole window.")
    },"additionalProperties":false}),
    OrchesterTool::UiLogs => json!({"type":"object","properties":{
      "lines":{"type":["integer","null"],"minimum":1,"maximum":MAX_LOG_LINES,
        "description":format!("The most lines, the latest ones; null for {DEFAULT_LOG_LINES}.")},
      "filter":query("Only lines holding this; null for all.")
    },"additionalProperties":false}),
    OrchesterTool::UiWait => json!({"type":"object","properties":{
      "condition":{"type":"string","enum":["idle","element","selector","text","url"],
        "description":"idle: rendered with nothing more to draw; element: an element query names is shown; for a web service also selector, text and url."},
      "query":query("For element: what the element's name, id or class holds; for selector: a CSS selector; for text: what the page's text holds; for url: what its address holds; null for idle."),
      "timeout_ms":{"type":["integer","null"],"minimum":1,"maximum":MAX_WAIT_MS,
        "description":format!("The longest to wait, in milliseconds; null for {DEFAULT_WAIT_MS}.")}
    },"required":["condition"],"additionalProperties":false}),
    _ => json!({"type":"object","properties":{},"additionalProperties":false}),
  }
}
