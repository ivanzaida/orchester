//! The values a browser tool's input carries, each checked when the call is read: a tab, an element's ref, a key
//! press, and the closed choices (mouse button, history step, scroll direction).

use serde::Deserialize;

/// The longest tab id a call may give.
const MAX_TAB_CHARS: usize = 64;
/// The most digits of an element ref's number.
const MAX_REF_DIGITS: usize = 9;
/// The longest key name (`ArrowDown`, `Backspace`, `F12`).
const MAX_KEY_CHARS: usize = 20;

/// One of the run's tabs, as `browser_tabs` names it: letters, digits, `-` and `_`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TabId(String);

impl TabId {
  pub fn new(value: &str) -> Result<Self, String> {
    let valid = (1..=MAX_TAB_CHARS).contains(&value.len())
      && value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if valid {
      Ok(Self(value.to_owned()))
    } else {
      Err(format!("`{value}` is not a tab: use an id browser_tabs lists"))
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// An element of the page as the tab's last `browser_read_page` named it: `e` and its number (`e12`). A ref of an
/// earlier read of the tab is stale; the runner says so.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ElementRef(String);

impl ElementRef {
  pub fn new(value: &str) -> Result<Self, String> {
    let valid = value
      .strip_prefix('e')
      .is_some_and(|number| (1..=MAX_REF_DIGITS).contains(&number.len()) && number.bytes().all(|b| b.is_ascii_digit()));
    if valid {
      Ok(Self(value.to_owned()))
    } else {
      Err(format!(
        "`{value}` is not an element ref: use a ref such as e12 from browser_read_page"
      ))
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// A key held during a key press.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Modifier {
  Ctrl,
  Alt,
  Shift,
  /// The Windows key, or ⌘ on macOS.
  Meta,
}

impl Modifier {
  fn from_name(name: &str) -> Option<Self> {
    Some(match name.to_ascii_lowercase().as_str() {
      "ctrl" | "control" => Self::Ctrl,
      "alt" | "option" => Self::Alt,
      "shift" => Self::Shift,
      "meta" | "cmd" | "command" => Self::Meta,
      _ => return None,
    })
  }
}

/// One key press with the modifiers held: `Enter`, `Tab`, `Ctrl+A`, `Shift+ArrowDown`, `Ctrl++`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyPress {
  modifiers: Vec<Modifier>,
  key: String,
}

impl KeyPress {
  /// `text` as modifiers and one key joined by `+`; the key is one character or a key's name of letters and digits.
  pub fn new(text: &str) -> Result<Self, String> {
    let invalid = || format!("`{text}` is not a key press such as Enter, Tab or Ctrl+A");
    let (held, key) = if text == "+" {
      ("", "+")
    } else if let Some(held) = text.strip_suffix("++") {
      (held, "+")
    } else {
      text.rsplit_once('+').unwrap_or(("", text))
    };
    let mut modifiers = Vec::new();
    for name in held.split('+').filter(|_| !held.is_empty()) {
      let modifier = Modifier::from_name(name).ok_or_else(invalid)?;
      if modifiers.contains(&modifier) {
        return Err(invalid());
      }
      modifiers.push(modifier);
    }
    let mut chars = key.chars();
    let single = chars.next().is_some_and(|c| !c.is_control() && !c.is_whitespace()) && chars.next().is_none();
    let named = (2..=MAX_KEY_CHARS).contains(&key.len()) && key.bytes().all(|b| b.is_ascii_alphanumeric());
    if !single && !named {
      return Err(invalid());
    }
    Ok(Self {
      modifiers,
      key: key.to_owned(),
    })
  }

  pub fn modifiers(&self) -> &[Modifier] {
    &self.modifiers
  }

  pub fn key(&self) -> &str {
    &self.key
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
  #[default]
  Left,
  Right,
  Middle,
}

/// A step in the tab's history, or a reload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryStep {
  Back,
  Forward,
  Reload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
  Up,
  Down,
  Left,
  Right,
}

/// Where a click lands: an element's centre, or a point of the page in CSS pixels.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ClickTarget {
  Element(ElementRef),
  Point { x: u32, y: u32 },
}

/// What a scroll does: brings an element into view, or moves the page by `pixels`, a screen when `None`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ScrollTarget {
  Element(ElementRef),
  Page {
    direction: ScrollDirection,
    pixels: Option<u32>,
  },
}

/// What `browser_wait` waits for.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum WaitFor {
  /// Literal text in the page.
  Text(String),
  Element(ElementRef),
}
