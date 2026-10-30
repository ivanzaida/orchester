//! The *Browser* window's messages (`docs/design/browser.md`, decisions 2 and 4, "The *Browser* window"): the app
//! never talks to the browser itself, it sends [`BrowserCommand`]s to the project's runner and gets
//! [`BrowserEvent`]s and screencast frames ([`ScreencastFrame`], a binary frame) back.

mod frames;
mod input;

use orchester_core::{id::RunId, time::Timestamp};
use serde::{Deserialize, Serialize};

pub use self::{
  frames::ScreencastFrame,
  input::{KeyAction, Modifiers, MouseAction, MouseButton, PersonInput},
};

/// A tab of the project's browser, by the id the browser gave it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BrowserTabId(String);

impl BrowserTabId {
  pub fn new(id: impl Into<String>) -> Self {
    Self(id.into())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// Whose a tab is: a run's, which only that run's agents reach, or the person's, which no agent sees (decision 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TabOwner {
  Run { run: RunId },
  Person,
}

/// One tab as the window lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserTab {
  pub id: BrowserTabId,
  pub owner: TabOwner,
  pub title: String,
  pub url: String,
  /// Its page is loading.
  pub loading: bool,
}

/// Where the project's browser draws: headless into the window's frames, or in a browser window of its own
/// (*Open in a browser window*, decision 7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserMode {
  #[default]
  Headless,
  Headed,
}

/// The project's browser as the window shows it: its tabs in the order they opened, the tab shown (screencast now, or
/// only named while the browser is in a window of its own), and where it draws. No tabs while it does not run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserState {
  pub tabs: Vec<BrowserTab>,
  pub shown: Option<BrowserTabId>,
  pub mode: BrowserMode,
}

/// The window's page area: its size in CSS pixels, the window's logical size, and its scale in percent (`200` on a
/// display of two pixels per point). Bounded, so a screencast never asks the browser for an absurd surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ViewportFields")]
pub struct Viewport {
  width: u32,
  height: u32,
  scale_percent: u32,
}

#[derive(Deserialize)]
struct ViewportFields {
  width: u32,
  height: u32,
  scale_percent: u32,
}

impl Viewport {
  /// The largest side, in CSS pixels.
  pub const MAX_SIDE: u32 = 8192;
  pub const MIN_SCALE_PERCENT: u32 = 50;
  pub const MAX_SCALE_PERCENT: u32 = 400;

  /// `None` for an empty side, a side above [`Self::MAX_SIDE`] or a scale outside its bounds.
  pub fn new(width: u32, height: u32, scale_percent: u32) -> Option<Self> {
    let sides = 1..=Self::MAX_SIDE;
    let scales = Self::MIN_SCALE_PERCENT..=Self::MAX_SCALE_PERCENT;
    (sides.contains(&width) && sides.contains(&height) && scales.contains(&scale_percent)).then_some(Self {
      width,
      height,
      scale_percent,
    })
  }

  pub const fn width(self) -> u32 {
    self.width
  }

  pub const fn height(self) -> u32 {
    self.height
  }

  pub const fn scale_percent(self) -> u32 {
    self.scale_percent
  }

  /// Device pixels per CSS pixel.
  pub fn scale(self) -> f64 {
    f64::from(self.scale_percent) / 100.0
  }
}

impl TryFrom<ViewportFields> for Viewport {
  type Error = String;

  fn try_from(fields: ViewportFields) -> Result<Self, Self::Error> {
    Self::new(fields.width, fields.height, fields.scale_percent).ok_or_else(|| {
      format!(
        "a viewport of {}x{} at {} % is outside its bounds",
        fields.width, fields.height, fields.scale_percent
      )
    })
  }
}

/// Where the person sends a tab: an address of the address line, or through its history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "to", rename_all = "snake_case")]
pub enum Navigation {
  Address { url: String },
  Back,
  Forward,
  Reload,
}

/// A JavaScript dialog of a page, which a headless frame does not show and the window draws instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialogKind {
  Alert,
  Confirm,
  Prompt,
  BeforeUnload,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserDialog {
  pub kind: DialogKind,
  pub message: String,
  /// What a prompt's field starts with.
  pub default_prompt: String,
}

/// A `<select>` element of a page, by the browser's id of its node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SelectId(u64);

impl SelectId {
  pub const fn new(node: u64) -> Self {
    Self(node)
  }

  pub const fn get(self) -> u64 {
    self.0
  }
}

/// An element's box in the page's CSS pixels, relative to the page area's top left corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementBox {
  pub x: i32,
  pub y: i32,
  pub width: u32,
  pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectOption {
  pub label: String,
  pub selected: bool,
  pub disabled: bool,
}

/// A `<select>` drop-down list the person opened: a headless frame does not show it, so the window draws its options
/// under the element's box and sends the choice back ([`BrowserCommand::ChooseOption`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectList {
  pub select: SelectId,
  pub options: Vec<SelectOption>,
  pub element: ElementBox,
}

/// What an agent did last on a tab, for *Agent is acting*: the tool, a short summary (*typed 8 in Max attempts*) and
/// the element it acted on, when there was one. The runner reports it, not CDP.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentAction {
  pub tool: String,
  pub summary: String,
  pub element: Option<ElementBox>,
}

/// The window's requests to the project's runner, each answered with a [`BrowserReply`] or a rejection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum BrowserCommand {
  /// The desktop follows the project's browser from now on: a [`BrowserEvent::Changed`] with its state follows at
  /// once and whenever it changes, with the other [`BrowserEvent`]s. Sent once a runner attached.
  Subscribe,
  /// Screencasts `tab`, at `viewport`, instead of the tab shown before; again with another viewport when the window's
  /// size or scale changes. Frames come as [`crate::Event::BrowserFrame`], only of this tab. With the browser in a
  /// window of its own no frames come: `tab` is only the one shown.
  Show {
    tab: BrowserTabId,
    viewport: Viewport,
  },
  /// Stops the screencast: the window closed, was minimised or shows another project.
  StopShowing,
  /// The person's `input` on `tab`.
  Input {
    tab: BrowserTabId,
    input: PersonInput,
  },
  Navigate {
    tab: BrowserTabId,
    to: Navigation,
  },
  /// Opens a tab of the person's (*+*) on `url`, or a blank one. Answered with [`BrowserReply::TabOpened`].
  OpenTab {
    url: Option<String>,
  },
  /// Closes `tab`, the person's or an agent's.
  CloseTab {
    tab: BrowserTabId,
  },
  /// Answers the JavaScript dialog of `tab`, with the text of a prompt.
  AnswerDialog {
    tab: BrowserTabId,
    accept: bool,
    text: Option<String>,
  },
  /// Chooses option `index` of the `<select>` list `select` of `tab`.
  ChooseOption {
    tab: BrowserTabId,
    select: SelectId,
    index: u32,
  },
  /// *Open in a browser window*: the browser starts again headed on the same profile and reopens the tabs by their
  /// URLs ([`BrowserMode::Headed`]).
  OpenInWindow,
  /// *Back to Orchester*: the browser starts again headless and reopens the tabs by their URLs.
  BackToOrchester,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum BrowserReply {
  /// The request was carried out.
  Done,
  /// The person's new tab.
  TabOpened { tab: BrowserTabId },
}

/// How an agent's wait for the person on a tab ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentWaitEnd {
  /// The person left the tab alone long enough, and the agent's call runs.
  Ran,
  /// The person was still using the tab when the wait reached its cap: the call answers without acting.
  Capped,
  /// The run stopped, or the tab went, while the call waited: it did not act.
  Cancelled,
}

/// What the project's browser tells a subscribed desktop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum BrowserEvent {
  /// The browser's state now: a tab opened, closed, loaded or changed its title or address, another tab is shown, or
  /// the browser started or ended.
  Changed(BrowserState),
  /// An agent of `run` opened the run's first tab: the window opens by itself if the person lets it (decision 14).
  AgentStartedBrowsing { run: RunId },
  /// An agent of `run` acted on `tab`, as `action` says.
  AgentActed {
    run: RunId,
    tab: BrowserTabId,
    action: AgentAction,
  },
  /// An agent of `run` waits on `tab` because the person uses it, until `until`; sent again when the person's input
  /// pushes that moment back.
  AgentWaiting {
    run: RunId,
    tab: BrowserTabId,
    until: Timestamp,
  },
  /// The wait of an agent of `run` on `tab` ended, as `end` says.
  AgentWaitEnded {
    run: RunId,
    tab: BrowserTabId,
    end: AgentWaitEnd,
  },
  /// A JavaScript dialog opened on the person's `tab`; it waits for [`BrowserCommand::AnswerDialog`].
  DialogOpened { tab: BrowserTabId, dialog: BrowserDialog },
  /// The dialog of `tab` closed, answered or not.
  DialogClosed { tab: BrowserTabId },
  /// The person opened a `<select>` list on `tab`.
  SelectOpened { tab: BrowserTabId, list: SelectList },
}
