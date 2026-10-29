//! The typed input of each browser tool, read strictly from a call's arguments (unknown fields refused), then checked
//! into values ([`super::values`]): a URL that is not an http or https page is refused with its reason
//! ([`BlockedUrl`]).

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use super::{
  BlockedUrl, BrowserTool, ClickTarget, ElementRef, HistoryStep, KeyPress, MouseButton, PageUrl, ScrollDirection,
  ScrollTarget, TabId, WaitFor,
};

pub(super) const DEFAULT_MAX_CHARS: u32 = 20_000;
pub(super) const MAX_MAX_CHARS: u32 = 50_000;
pub(super) const DEFAULT_LIMIT: u32 = 50;
pub(super) const MAX_LIMIT: u32 = 500;
pub(super) const DEFAULT_WAIT_SECONDS: u32 = 10;
pub(super) const MAX_WAIT_SECONDS: u32 = 60;
pub(super) const MAX_PIXELS: u32 = 100_000;
pub(super) const MAX_KEYS: usize = 20;
pub(super) const MAX_TEXT_CHARS: usize = 10_000;
pub(super) const MAX_PATTERN_CHARS: usize = 200;
pub(super) const MAX_EXPRESSION_CHARS: usize = 10_000;

/// Why a browser call is not run. The message is the error result the agent gets.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum BrowserCallError {
  #[error("there is no browser tool named `{0}`")]
  UnknownTool(String),
  #[error("invalid input for `{tool}`: {message}", tool = .tool.as_str())]
  Invalid { tool: BrowserTool, message: String },
  #[error(transparent)]
  Blocked(#[from] BlockedUrl),
}

/// One browser call, read into its tool's input. `tab` is `None` for the run's current tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserCall {
  Tabs,
  Open {
    url: PageUrl,
    new_tab: bool,
  },
  Navigate {
    tab: Option<TabId>,
    to: HistoryStep,
  },
  Close {
    tab: Option<TabId>,
  },
  ReadPage {
    tab: Option<TabId>,
    query: Option<String>,
    interactive_only: bool,
    max_chars: u32,
  },
  PageText {
    tab: Option<TabId>,
    max_chars: u32,
  },
  Click {
    tab: Option<TabId>,
    target: ClickTarget,
    button: MouseButton,
    count: u8,
  },
  Type {
    tab: Option<TabId>,
    element: ElementRef,
    text: String,
    submit: bool,
  },
  Key {
    tab: Option<TabId>,
    keys: Vec<KeyPress>,
  },
  Scroll {
    tab: Option<TabId>,
    target: ScrollTarget,
  },
  Screenshot {
    tab: Option<TabId>,
    element: Option<ElementRef>,
    full_page: bool,
  },
  Console {
    tab: Option<TabId>,
    errors_only: bool,
    pattern: Option<String>,
    limit: u32,
  },
  Network {
    tab: Option<TabId>,
    url_pattern: Option<String>,
    limit: u32,
  },
  Wait {
    tab: Option<TabId>,
    until: WaitFor,
    timeout_seconds: u32,
  },
  Eval {
    tab: Option<TabId>,
    expression: String,
  },
}

/// Every field any browser tool takes; [`fields_of`] says which a tool may give, the rest are refused.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fields {
  tab: Option<String>,
  url: Option<String>,
  new_tab: Option<bool>,
  to: Option<HistoryStep>,
  query: Option<String>,
  interactive_only: Option<bool>,
  max_chars: Option<u32>,
  #[serde(rename = "ref")]
  element: Option<String>,
  x: Option<u32>,
  y: Option<u32>,
  button: Option<MouseButton>,
  count: Option<u8>,
  text: Option<String>,
  submit: Option<bool>,
  keys: Option<Vec<String>>,
  direction: Option<ScrollDirection>,
  pixels: Option<u32>,
  full_page: Option<bool>,
  errors_only: Option<bool>,
  pattern: Option<String>,
  url_pattern: Option<String>,
  limit: Option<u32>,
  timeout: Option<u32>,
  expression: Option<String>,
}

/// Why `arguments` are not a tool's input: invalid, or a URL that is blocked.
enum Refusal {
  Invalid(String),
  Blocked(BlockedUrl),
}

impl From<String> for Refusal {
  fn from(message: String) -> Self {
    Self::Invalid(message)
  }
}

impl From<&str> for Refusal {
  fn from(message: &str) -> Self {
    Self::Invalid(message.to_owned())
  }
}

impl From<BlockedUrl> for Refusal {
  fn from(blocked: BlockedUrl) -> Self {
    Self::Blocked(blocked)
  }
}

impl BrowserCall {
  /// The call of browser tool `name` with `arguments`; no arguments is a call with none (`{}`). Whether the step is
  /// offered the tool and whether the call asks is [`super::decide`]'s.
  pub fn parse(name: &str, arguments: &Value) -> Result<Self, BrowserCallError> {
    let tool = BrowserTool::from_name(name).ok_or_else(|| BrowserCallError::UnknownTool(name.to_owned()))?;
    let call = read_fields(tool, arguments)
      .map_err(Refusal::from)
      .and_then(|fields| read(tool, fields));
    call.map_err(|refusal| match refusal {
      Refusal::Invalid(message) => BrowserCallError::Invalid { tool, message },
      Refusal::Blocked(blocked) => BrowserCallError::Blocked(blocked),
    })
  }

  pub const fn tool(&self) -> BrowserTool {
    match self {
      Self::Tabs => BrowserTool::Tabs,
      Self::Open { .. } => BrowserTool::Open,
      Self::Navigate { .. } => BrowserTool::Navigate,
      Self::Close { .. } => BrowserTool::Close,
      Self::ReadPage { .. } => BrowserTool::ReadPage,
      Self::PageText { .. } => BrowserTool::PageText,
      Self::Click { .. } => BrowserTool::Click,
      Self::Type { .. } => BrowserTool::Type,
      Self::Key { .. } => BrowserTool::Key,
      Self::Scroll { .. } => BrowserTool::Scroll,
      Self::Screenshot { .. } => BrowserTool::Screenshot,
      Self::Console { .. } => BrowserTool::Console,
      Self::Network { .. } => BrowserTool::Network,
      Self::Wait { .. } => BrowserTool::Wait,
      Self::Eval { .. } => BrowserTool::Eval,
    }
  }

  /// The URL a `browser_open` call opens, whose host decides whether it asks; the runner knows the page of the tab
  /// any other call acts on.
  pub const fn opened_url(&self) -> Option<&PageUrl> {
    match self {
      Self::Open { url, .. } => Some(url),
      _ => None,
    }
  }
}

/// The fields of `arguments`, refused when the tool does not take one of them.
fn read_fields(tool: BrowserTool, arguments: &Value) -> Result<Fields, String> {
  let Some(object) = arguments.as_object() else {
    return if arguments.is_null() {
      Ok(Fields::default())
    } else {
      Err("the arguments are an object".to_owned())
    };
  };
  let taken = fields_of(tool);
  if let Some(field) = object.keys().find(|field| !taken.contains(&field.as_str())) {
    return Err(format!("unknown field `{field}`, expected one of {}", taken.join(", ")));
  }
  serde_json::from_value(arguments.clone()).map_err(|error| error.to_string())
}

/// The fields `tool` takes, as its schema lists them.
fn fields_of(tool: BrowserTool) -> &'static [&'static str] {
  match tool {
    BrowserTool::Tabs => &[],
    BrowserTool::Open => &["url", "new_tab"],
    BrowserTool::Navigate => &["tab", "to"],
    BrowserTool::Close => &["tab"],
    BrowserTool::ReadPage => &["tab", "query", "interactive_only", "max_chars"],
    BrowserTool::PageText => &["tab", "max_chars"],
    BrowserTool::Click => &["tab", "ref", "x", "y", "button", "count"],
    BrowserTool::Type => &["tab", "ref", "text", "submit"],
    BrowserTool::Key => &["tab", "keys"],
    BrowserTool::Scroll => &["tab", "ref", "direction", "pixels"],
    BrowserTool::Screenshot => &["tab", "ref", "full_page"],
    BrowserTool::Console => &["tab", "errors_only", "pattern", "limit"],
    BrowserTool::Network => &["tab", "url_pattern", "limit"],
    BrowserTool::Wait => &["tab", "text", "ref", "timeout"],
    BrowserTool::Eval => &["tab", "expression"],
  }
}

/// `fields` read as the input of `tool`.
fn read(tool: BrowserTool, fields: Fields) -> Result<BrowserCall, Refusal> {
  let tab = fields.tab.as_deref().map(TabId::new).transpose()?;
  Ok(match tool {
    BrowserTool::Tabs => BrowserCall::Tabs,
    BrowserTool::Open => BrowserCall::Open {
      url: PageUrl::parse(fields.url.as_deref().ok_or("give the url")?)?,
      new_tab: fields.new_tab.unwrap_or(false),
    },
    BrowserTool::Navigate => BrowserCall::Navigate {
      tab,
      to: fields.to.ok_or("give to: back, forward or reload")?,
    },
    BrowserTool::Close => BrowserCall::Close { tab },
    BrowserTool::ReadPage => BrowserCall::ReadPage {
      tab,
      query: pattern(fields.query, "query")?,
      interactive_only: fields.interactive_only.unwrap_or(false),
      max_chars: bounded(fields.max_chars, DEFAULT_MAX_CHARS, MAX_MAX_CHARS, "max_chars")?,
    },
    BrowserTool::PageText => BrowserCall::PageText {
      tab,
      max_chars: bounded(fields.max_chars, DEFAULT_MAX_CHARS, MAX_MAX_CHARS, "max_chars")?,
    },
    BrowserTool::Click => click(tab, &fields)?,
    BrowserTool::Type => BrowserCall::Type {
      tab,
      element: ElementRef::new(fields.element.as_deref().ok_or("give the field's ref")?)?,
      text: text(fields.text.ok_or("give the text")?, MAX_TEXT_CHARS, "text")?,
      submit: fields.submit.unwrap_or(false),
    },
    BrowserTool::Key => BrowserCall::Key {
      tab,
      keys: keys(fields.keys.as_deref().ok_or("give the keys")?)?,
    },
    BrowserTool::Scroll => BrowserCall::Scroll {
      tab,
      target: scroll(&fields)?,
    },
    BrowserTool::Screenshot => screenshot(tab, &fields)?,
    BrowserTool::Console => BrowserCall::Console {
      tab,
      errors_only: fields.errors_only.unwrap_or(false),
      pattern: pattern(fields.pattern, "pattern")?,
      limit: bounded(fields.limit, DEFAULT_LIMIT, MAX_LIMIT, "limit")?,
    },
    BrowserTool::Network => BrowserCall::Network {
      tab,
      url_pattern: pattern(fields.url_pattern, "url_pattern")?,
      limit: bounded(fields.limit, DEFAULT_LIMIT, MAX_LIMIT, "limit")?,
    },
    BrowserTool::Wait => wait(tab, fields)?,
    BrowserTool::Eval => BrowserCall::Eval {
      tab,
      expression: text(
        fields.expression.ok_or("give the expression")?,
        MAX_EXPRESSION_CHARS,
        "expression",
      )?,
    },
  })
}

fn click(tab: Option<TabId>, fields: &Fields) -> Result<BrowserCall, String> {
  let target = match (&fields.element, fields.x, fields.y) {
    (Some(element), None, None) => ClickTarget::Element(ElementRef::new(element)?),
    (None, Some(x), Some(y)) if x <= MAX_PIXELS && y <= MAX_PIXELS => ClickTarget::Point { x, y },
    _ => return Err(format!("give either ref, or both x and y of at most {MAX_PIXELS}")),
  };
  let count = fields.count.unwrap_or(1);
  if !(1..=3).contains(&count) {
    return Err("count is 1, 2 or 3".to_owned());
  }
  Ok(BrowserCall::Click {
    tab,
    target,
    button: fields.button.unwrap_or_default(),
    count,
  })
}

fn scroll(fields: &Fields) -> Result<ScrollTarget, String> {
  match (&fields.element, fields.direction) {
    (Some(element), None) if fields.pixels.is_none() => Ok(ScrollTarget::Element(ElementRef::new(element)?)),
    (None, Some(direction)) => Ok(ScrollTarget::Page {
      direction,
      pixels: fields
        .pixels
        .map(|pixels| bounded(Some(pixels), pixels, MAX_PIXELS, "pixels"))
        .transpose()?,
    }),
    _ => Err("give either ref, or a direction with pixels or none".to_owned()),
  }
}

fn screenshot(tab: Option<TabId>, fields: &Fields) -> Result<BrowserCall, String> {
  let element = fields.element.as_deref().map(ElementRef::new).transpose()?;
  let full_page = fields.full_page.unwrap_or(false);
  if element.is_some() && full_page {
    return Err("give ref or full_page, not both".to_owned());
  }
  Ok(BrowserCall::Screenshot {
    tab,
    element,
    full_page,
  })
}

fn wait(tab: Option<TabId>, fields: Fields) -> Result<BrowserCall, String> {
  let until = match (fields.text, fields.element) {
    (Some(text), None) => WaitFor::Text(pattern(Some(text), "text")?.ok_or("give the text to wait for")?),
    (None, Some(element)) => WaitFor::Element(ElementRef::new(&element)?),
    _ => return Err("give either text or ref".to_owned()),
  };
  Ok(BrowserCall::Wait {
    tab,
    until,
    timeout_seconds: bounded(fields.timeout, DEFAULT_WAIT_SECONDS, MAX_WAIT_SECONDS, "timeout")?,
  })
}

fn keys(names: &[String]) -> Result<Vec<KeyPress>, String> {
  if names.is_empty() || names.len() > MAX_KEYS {
    return Err(format!("give 1 to {MAX_KEYS} key presses"));
  }
  names.iter().map(|name| KeyPress::new(name)).collect()
}

/// `value` within 1 and `max`, `default` when it is none.
fn bounded(value: Option<u32>, default: u32, max: u32, field: &str) -> Result<u32, String> {
  match value.unwrap_or(default) {
    0 => Err(format!("{field} is at least 1")),
    value if value > max => Err(format!("{field} is at most {max}")),
    value => Ok(value),
  }
}

/// Literal text to look for: one line, at most [`MAX_PATTERN_CHARS`]; blank is none.
fn pattern(value: Option<String>, field: &str) -> Result<Option<String>, String> {
  let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
    return Ok(None);
  };
  if value.chars().count() > MAX_PATTERN_CHARS || value.contains(['\n', '\r']) {
    return Err(format!("{field} is one line of at most {MAX_PATTERN_CHARS} characters"));
  }
  Ok(Some(value))
}

fn text(value: String, max: usize, field: &str) -> Result<String, String> {
  if value.chars().count() > max {
    return Err(format!("{field} is at most {max} characters"));
  }
  Ok(value)
}
