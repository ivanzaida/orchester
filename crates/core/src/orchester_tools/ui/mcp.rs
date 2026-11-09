//! The `mcp` UI driver (`docs/architecture/agent-testing.md`, "UI drivers"): an app with its own MCP server, such as
//! lurq apps and Orchester, is seen and used through its lurq MCP tools, over the run's relay to the app
//! (`call_run_app`'s). Each `ui_*` call is one request ([`request`]): `ui_open` lists the app's windows
//! (`lurq_windows`), `ui_snapshot` inspects it (`lurq_inspect`), `ui_act` acts by ref (`lurq_act`, `lurq_interact`,
//! `lurq_set_value`), `ui_screenshot` captures its window (`lurq_screenshot`), `ui_logs` reads its log
//! (`lurq_logs`) and `ui_wait` waits for it to render (`lurq_wait`) or inspects it until the element is there. The
//! app's answer is read into the shape every driver answers in ([`answer`]). Clicks go through the app's own
//! semantic action and text through its synthetic input, both inside its window: nothing reaches the computer's input.

use serde_json::{Value, json};

use super::{UiAction, UiCall, UiCondition, UiElement};

/// The most elements one snapshot inspects: a whole window of the app, kept well within an answer's size.
const SNAPSHOT_NODES: u32 = 300;
/// How deep one snapshot inspects.
const SNAPSHOT_DEPTH: u32 = 40;
/// How long a wait for an element waits between two looks, in milliseconds.
pub const WAIT_POLL_MS: u64 = 250;

/// One request to the app: its MCP tool and that tool's arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppRequest {
  pub tool: &'static str,
  pub arguments: Value,
}

/// What the app's answer to a call comes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiAnswer {
  /// The call's answer, in the common shape.
  Done(Value),
  /// A wait for an element that is not shown yet: look again until the wait's time is over.
  NotYet,
}

/// The app's request for `call`.
pub fn request(call: &UiCall) -> AppRequest {
  let (tool, arguments) = match call {
    UiCall::Open(_) => ("lurq_windows", json!({})),
    UiCall::Snapshot { query } => ("lurq_inspect", inspect(query.as_deref())),
    UiCall::Act { target, action } => {
      let reference = target.as_ref().map(super::UiRef::as_str);
      match action {
        UiAction::Click => ("lurq_act", json!({"ref": reference, "action": "invoke"})),
        UiAction::Type(text) => (
          "lurq_interact",
          json!({"action": "type", "ref": reference, "text": text}),
        ),
        UiAction::Key(key) => {
          let mut arguments = json!({"action": "key", "key": key});
          if let Some(reference) = reference {
            arguments["ref"] = json!(reference);
          }
          ("lurq_interact", arguments)
        }
        UiAction::Scroll => ("lurq_interact", json!({"action": "scroll_to", "ref": reference})),
        UiAction::Select(value) => ("lurq_set_value", json!({"ref": reference, "value": value})),
      }
    }
    UiCall::Screenshot { target } => (
      "lurq_screenshot",
      target
        .as_ref()
        .map_or_else(|| json!({}), |target| json!({"ref": target.as_str()})),
    ),
    UiCall::Logs { lines, filter } => {
      let mut arguments = json!({"lines": lines});
      if let Some(filter) = filter {
        arguments["filter"] = json!(filter);
      }
      ("lurq_logs", arguments)
    }
    UiCall::Wait {
      condition: UiCondition::Idle,
      timeout_ms,
    } => ("lurq_wait", json!({"timeout_ms": timeout_ms})),
    // A page's conditions are the browser driver's: the runner refuses them before an app is asked. Read as an element
    // here, an app would wait for an element named so.
    UiCall::Wait {
      condition:
        UiCondition::Element(query) | UiCondition::Selector(query) | UiCondition::Text(query) | UiCondition::Url(query),
      ..
    } => ("lurq_inspect", inspect(Some(query))),
  };
  AppRequest { tool, arguments }
}

/// `lurq_inspect`'s arguments: the whole window, or only what `query` names.
fn inspect(query: Option<&str>) -> Value {
  let mut arguments = json!({"max_nodes": SNAPSHOT_NODES, "max_depth": SNAPSHOT_DEPTH});
  if let Some(query) = query {
    arguments["query"] = json!(query);
  }
  arguments
}

/// The app's MCP `tools/call` result `result` for `call`, read into the common shape; the app's own words when it
/// answered with an error.
pub fn answer(call: &UiCall, result: &Value) -> Result<UiAnswer, String> {
  if result["isError"].as_bool().unwrap_or(false) {
    let said = texts(result).join("\n");
    return Err(if said.is_empty() {
      "the app answered with an error and no text".to_owned()
    } else {
      said
    });
  }
  let payload = payload(result);
  Ok(UiAnswer::Done(match call {
    UiCall::Open(_) => json!({"windows": payload.get("windows").cloned().unwrap_or(payload)}),
    UiCall::Snapshot { .. } => snapshot(&payload),
    UiCall::Act { target, action } => json!({"acted": action.as_str(), "ref": target}),
    UiCall::Screenshot { .. } => json!({"screenshot": saved_png(result)?}),
    UiCall::Logs { .. } => json!({"lines": log_lines(result)}),
    UiCall::Wait {
      condition: UiCondition::Idle,
      ..
    } => json!({"met": "idle"}),
    UiCall::Wait {
      condition: UiCondition::Element(_) | UiCondition::Selector(_) | UiCondition::Text(_) | UiCondition::Url(_),
      ..
    } => {
      let matches = elements(&payload["matches"]);
      if matches.is_empty() {
        return Ok(UiAnswer::NotYet);
      }
      json!({"met": "element", "matches": matches})
    }
  }))
}

/// The text blocks of `result`.
fn texts(result: &Value) -> Vec<&str> {
  result["content"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(|block| block["text"].as_str())
    .collect()
}

/// What the app answered as data: its structured content, or its first text block read as JSON; `null` for neither.
fn payload(result: &Value) -> Value {
  if let Some(structured) = result.get("structuredContent").filter(|value| !value.is_null()) {
    return structured.clone();
  }
  texts(result)
    .first()
    .and_then(|text| serde_json::from_str(text).ok())
    .unwrap_or(Value::Null)
}

/// A snapshot in the common shape: `lurq_inspect`'s tree, or its matches for a query.
fn snapshot(payload: &Value) -> Value {
  let truncated = payload["truncated"].as_bool().unwrap_or(false);
  if let Some(matches) = payload.get("matches") {
    return json!({"matches": elements(matches), "truncated": truncated});
  }
  json!({"tree": element(&payload["tree"]), "truncated": truncated})
}

/// The elements of a list of inspected nodes, without their children.
fn elements(nodes: &Value) -> Vec<UiElement> {
  nodes
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(element)
    .map(|element| UiElement {
      children: Vec::new(),
      ..element
    })
    .collect()
}

/// An inspected node as an element with what is inside it; `None` for a node without a ref.
fn element(node: &Value) -> Option<UiElement> {
  let reference = node["ref"].as_str()?.to_owned();
  Some(UiElement {
    reference,
    role: node["role"].as_str().unwrap_or("generic").to_owned(),
    name: node["name"].as_str().filter(|name| !name.is_empty()).map(str::to_owned),
    states: Vec::new(),
    value: None,
    children: node["children"]
      .as_array()
      .into_iter()
      .flatten()
      .filter_map(element)
      .collect(),
  })
}

/// The path the relay saved a screenshot at, from its `PNG saved: <path>; open it with read_file to see it` block.
fn saved_png(result: &Value) -> Result<String, String> {
  texts(result)
    .into_iter()
    .find_map(|text| text.strip_prefix("PNG saved: ")?.split("; open it").next())
    .map(str::to_owned)
    .ok_or_else(|| "the app answered no screenshot".to_owned())
}

/// The log lines `lurq_logs` answered, newest last; none when it matched none.
fn log_lines(result: &Value) -> Vec<String> {
  texts(result)
    .into_iter()
    .flat_map(str::lines)
    .filter(|line| *line != "(no matching log lines)")
    .map(str::to_owned)
    .collect()
}
