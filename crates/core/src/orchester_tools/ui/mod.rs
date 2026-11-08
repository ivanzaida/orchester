//! The `ui_*` tools (`docs/architecture/agent-testing.md`, "UI drivers"): one interface for seeing and using the UI
//! of a service the step started, whatever drives it underneath. `ui_open(service)` picks the service the later calls
//! reach; `ui_snapshot()` answers its UI as a tree of elements, each with a ref, a role and a name ([`UiElement`]);
//! `ui_act(ref, action)` clicks, types into, presses a key on, scrolls to or selects in one of them ([`UiAction`]);
//! `ui_screenshot()` saves a PNG of its window in the run's storage and answers the path, which `read_file` shows as
//! the image; `ui_logs()` answers its latest log lines; and `ui_wait(condition)` waits until it rendered or until an
//! element appears ([`UiCondition`]), and for a web page until a selector matches, its text holds a query or its
//! address does. Every driver answers in this shape; the `mcp` driver ([`mcp`]) maps it onto an app's own lurq MCP
//! tools, and the `browser` driver's tab opens only the services' origins ([`origins`]).
//!
//! The rules hold for every driver: only an app this run started, never global input (an app's synthetic input
//! reaches only its own window), never a whole-screen capture (only the app's window).

mod catalog;
pub mod mcp;
pub mod origins;

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(super) use self::catalog::{description, input_schema};
use super::OrchesterTool;
use crate::service::ServiceName;

/// The longest ref a call may give.
pub const MAX_REF_CHARS: usize = 64;
/// The longest text `ui_act` types, presses or selects.
pub const MAX_TEXT_CHARS: usize = 10_000;
/// The longest query `ui_snapshot` and `ui_wait` take.
pub const MAX_QUERY_CHARS: usize = 200;
/// How long `ui_wait` waits when the call names no time.
pub const DEFAULT_WAIT_MS: u32 = 10_000;
/// The longest `ui_wait` waits.
pub const MAX_WAIT_MS: u32 = 60_000;
/// The log lines `ui_logs` answers when the call names no number.
pub const DEFAULT_LOG_LINES: u32 = 200;
/// The most log lines `ui_logs` answers.
pub const MAX_LOG_LINES: u32 = 1000;

/// An element's handle from the latest `ui_snapshot`: letters, digits, `_`, `-`, `:` and `.`, as drivers mint them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct UiRef(String);

impl UiRef {
  pub fn new(value: &str) -> Result<Self, String> {
    let valid = !value.is_empty()
      && value.chars().count() <= MAX_REF_CHARS
      && value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '.'));
    if valid {
      Ok(Self(value.to_owned()))
    } else {
      Err(format!("`{value}` is not a ref from ui_snapshot"))
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// What `ui_act` does to an element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiAction {
  /// Clicks it, as a person's click on it would.
  Click,
  /// Focuses it and types the text into it.
  Type(String),
  /// Presses the key, e.g. `Enter`, `Tab`, `ArrowDown` or `a`, in the element's window.
  Key(String),
  /// Scrolls it into view.
  Scroll,
  /// Selects the option with this label in it, or sets the value of a field.
  Select(String),
}

impl UiAction {
  /// The action's name, as a call gives it.
  pub const fn as_str(&self) -> &'static str {
    match self {
      Self::Click => "click",
      Self::Type(_) => "type",
      Self::Key(_) => "key",
      Self::Scroll => "scroll",
      Self::Select(_) => "select",
    }
  }
}

/// What `ui_wait` waits for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiCondition {
  /// The UI rendered and has nothing more to draw.
  Idle,
  /// An element whose name, id or class holds this text is shown.
  Element(String),
  /// A web page holds an element this CSS selector matches; only the `browser` driver reads a page's selectors.
  Selector(String),
  /// A web page's text holds this; only the `browser` driver reads a page's text.
  Text(String),
  /// A web page's address holds this; only the `browser` driver has an address.
  Url(String),
}

impl UiCondition {
  /// The condition's name, as a call gives it.
  pub const fn as_str(&self) -> &'static str {
    match self {
      Self::Idle => "idle",
      Self::Element(_) => "element",
      Self::Selector(_) => "selector",
      Self::Text(_) => "text",
      Self::Url(_) => "url",
    }
  }

  /// Whether only a web page can hold it: the `browser` driver's own conditions.
  pub const fn is_page(&self) -> bool {
    matches!(self, Self::Selector(_) | Self::Text(_) | Self::Url(_))
  }
}

/// One call of a `ui_*` tool, read into its input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiCall {
  /// `ui_open`: the service the later calls reach.
  Open(ServiceName),
  /// `ui_snapshot`: the element tree, or only the elements `query` names.
  Snapshot { query: Option<String> },
  /// `ui_act`: `action` on the element `target`; a key goes to the focused element when it names none.
  Act { target: Option<UiRef>, action: UiAction },
  /// `ui_screenshot`: the window, or only the element `target`.
  Screenshot { target: Option<UiRef> },
  /// `ui_logs`: the latest `lines` log lines, only those holding `filter` when it is given.
  Logs { lines: u32, filter: Option<String> },
  /// `ui_wait`: until `condition` holds, at most `timeout_ms`.
  Wait { condition: UiCondition, timeout_ms: u32 },
}

/// An element of a snapshot, the shape every driver answers in: its ref, its role, its accessible name when it has
/// one, its states (`disabled`, `focused`, `checked`, `expanded`, `selected`, `readonly`, …) and its value when its
/// driver reads them, and the elements inside it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UiElement {
  #[serde(rename = "ref")]
  pub reference: String,
  pub role: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub states: Vec<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub value: Option<String>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub children: Vec<Self>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenInput {
  service: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotInput {
  #[serde(default)]
  query: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActInput {
  #[serde(rename = "ref", default)]
  reference: Option<String>,
  action: String,
  #[serde(default)]
  text: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScreenshotInput {
  #[serde(rename = "ref", default)]
  reference: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LogsInput {
  #[serde(default)]
  lines: Option<NonZeroU32>,
  #[serde(default)]
  filter: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WaitInput {
  condition: String,
  #[serde(default)]
  query: Option<String>,
  #[serde(default)]
  timeout_ms: Option<NonZeroU32>,
}

impl UiCall {
  /// `arguments` read as the input of the `ui_*` tool `tool`, or why they are not.
  pub(super) fn parse(tool: OrchesterTool, arguments: Value) -> Result<Self, String> {
    let invalid = |error: serde_json::Error| error.to_string();
    Ok(match tool {
      OrchesterTool::UiOpen => {
        let input: OpenInput = serde_json::from_value(arguments).map_err(invalid)?;
        Self::Open(ServiceName::new(&input.service).map_err(|error| error.to_string())?)
      }
      OrchesterTool::UiSnapshot => {
        let input: SnapshotInput = serde_json::from_value(arguments).map_err(invalid)?;
        Self::Snapshot {
          query: input.query.as_deref().map(query).transpose()?,
        }
      }
      OrchesterTool::UiAct => act(&serde_json::from_value(arguments).map_err(invalid)?)?,
      OrchesterTool::UiScreenshot => {
        let input: ScreenshotInput = serde_json::from_value(arguments).map_err(invalid)?;
        Self::Screenshot {
          target: input.reference.as_deref().map(UiRef::new).transpose()?,
        }
      }
      OrchesterTool::UiLogs => {
        let input: LogsInput = serde_json::from_value(arguments).map_err(invalid)?;
        let lines = input.lines.map_or(DEFAULT_LOG_LINES, NonZeroU32::get);
        if lines > MAX_LOG_LINES {
          return Err(format!("lines is at most {MAX_LOG_LINES}"));
        }
        Self::Logs {
          lines,
          filter: input.filter.as_deref().map(query).transpose()?,
        }
      }
      OrchesterTool::UiWait => wait(&serde_json::from_value(arguments).map_err(invalid)?)?,
      other => return Err(format!("{} is not a ui tool", other.as_str())),
    })
  }

  /// The tool the call is of.
  pub const fn tool(&self) -> OrchesterTool {
    match self {
      Self::Open(_) => OrchesterTool::UiOpen,
      Self::Snapshot { .. } => OrchesterTool::UiSnapshot,
      Self::Act { .. } => OrchesterTool::UiAct,
      Self::Screenshot { .. } => OrchesterTool::UiScreenshot,
      Self::Logs { .. } => OrchesterTool::UiLogs,
      Self::Wait { .. } => OrchesterTool::UiWait,
    }
  }

  /// The service `ui_open` names; `None` for the other calls.
  pub const fn opened(&self) -> Option<&ServiceName> {
    match self {
      Self::Open(name) => Some(name),
      _ => None,
    }
  }
}

/// A `ui_act` call: `click`, `scroll`, `type` and `select` on a ref, `key` on a ref or the focused element; `type`,
/// `key` and `select` with their `text`.
fn act(input: &ActInput) -> Result<UiCall, String> {
  let target = input.reference.as_deref().map(UiRef::new).transpose()?;
  let text = || {
    let text = input
      .text
      .clone()
      .ok_or_else(|| format!("{} needs text", input.action))?;
    if text.is_empty() || text.chars().count() > MAX_TEXT_CHARS {
      return Err(format!("text is 1 to {MAX_TEXT_CHARS} characters"));
    }
    Ok(text)
  };
  let action = match input.action.as_str() {
    "click" => UiAction::Click,
    "type" => UiAction::Type(text()?),
    "key" => UiAction::Key(text()?),
    "scroll" => UiAction::Scroll,
    "select" => UiAction::Select(text()?),
    other => {
      return Err(format!(
        "`{other}` is not an action: click, type, key, scroll or select"
      ));
    }
  };
  if target.is_none() && !matches!(action, UiAction::Key(_)) {
    return Err(format!(
      "{} needs the ref of an element from ui_snapshot",
      action.as_str()
    ));
  }
  if !matches!(action, UiAction::Type(_) | UiAction::Key(_) | UiAction::Select(_)) && input.text.is_some() {
    return Err(format!("{} takes no text", action.as_str()));
  }
  Ok(UiCall::Act { target, action })
}

/// A `ui_wait` call: `idle`, or `element`, `selector`, `text` or `url` with the query it waits for.
fn wait(input: &WaitInput) -> Result<UiCall, String> {
  let timeout_ms = input.timeout_ms.map_or(DEFAULT_WAIT_MS, NonZeroU32::get);
  if timeout_ms > MAX_WAIT_MS {
    return Err(format!("timeout_ms is at most {MAX_WAIT_MS}"));
  }
  let condition = match (input.condition.as_str(), input.query.as_deref()) {
    ("idle", None) => UiCondition::Idle,
    ("idle", Some(_)) => return Err("idle takes no query".to_owned()),
    ("element", Some(text)) => UiCondition::Element(query(text)?),
    ("selector", Some(text)) => UiCondition::Selector(query(text)?),
    ("text", Some(text)) => UiCondition::Text(query(text)?),
    ("url", Some(text)) => UiCondition::Url(query(text)?),
    ("element", None) => return Err("element needs the query of the element to wait for".to_owned()),
    (condition @ ("selector" | "text" | "url"), None) => {
      return Err(format!("{condition} needs the query it waits for"));
    }
    (other, _) => {
      return Err(format!(
        "`{other}` is not a condition: idle, element, selector, text or url"
      ));
    }
  };
  Ok(UiCall::Wait { condition, timeout_ms })
}

/// A query or a filter as a call gives it: one line of 1 to [`MAX_QUERY_CHARS`] characters.
fn query(text: &str) -> Result<String, String> {
  if text.trim().is_empty() || text.chars().count() > MAX_QUERY_CHARS || text.chars().any(char::is_control) {
    return Err(format!("`{text}` is not one line of 1 to {MAX_QUERY_CHARS} characters"));
  }
  Ok(text.to_owned())
}
