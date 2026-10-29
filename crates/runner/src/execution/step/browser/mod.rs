//! The browser tools (`orchester_core::orchester_tools::browser`, `docs/design/browser.md`, "What the agent gets"),
//! served through the invocation's relay beside Orchester's other tools ([`super::relay`]) to a step whose profile has
//! *Browser* on and which has network access, read-only steps included. Each call reaches only the run's own tabs in
//! the project's browser ([`crate::browser::ProjectBrowser`]), which its first call starts.
//!
//! Whether a call runs or asks is decided per call ([`StepBrowser::look`]), by P1's policy on the site it goes to or
//! acts on: the URL `browser_open` opens, else the page of the tab it acts on. The reading tools and `browser_close`
//! run; opening or acting on an allowed site or a local development host runs; any other site, or a page without a
//! web host, asks; `browser_eval` asks at every call. The relay asks the person itself, as for any of Orchester's tools
//! that asks, so every CLI runs these tools unasked.
//!
//! Every answer with page content starts with [`PAGE_DATA_NOTICE`], and a JavaScript dialog the page opened is
//! dismissed and named in the next answer on its tab. Nobody watches the browser yet: the person's *Browser* window is
//! the design's P4.

mod act;
mod keys;
mod logs;
mod navigation;
mod page;
mod script;
mod shot;

use std::{fmt::Write as _, path::PathBuf, sync::Arc};

use orchester_core::{
  id::RunId,
  orchester_tools::{
    AnswerImage, ToolPolicy, ToolResult,
    browser::{self, BrowserCall, BrowserReach, PAGE_DATA_NOTICE, PageUrl, TabId},
  },
  permission::{AgentPermissions, browser::SiteHost},
};
use thiserror::Error;

use super::orchester_tools::Looked;
use crate::browser::{BrowserError, ProjectBrowser, Tab, cdp::TargetId};

/// Why a browser call did not do what it asked. The message is the error result the agent gets.
#[derive(Debug, Error)]
enum ToolError {
  #[error(transparent)]
  Browser(#[from] BrowserError),
  /// What the page or the browser answered instead: an element without a box, an exception, an unknown key.
  #[error("{0}")]
  Page(String),
  #[error("{what} did not appear within {seconds} s")]
  Waited { what: String, seconds: u32 },
  #[error("the screenshot was not saved in .tmp/browser/ of the worktree: {0}")]
  Save(String),
}

impl From<crate::browser::cdp::CdpError> for ToolError {
  fn from(error: crate::browser::cdp::CdpError) -> Self {
    Self::Browser(error.into())
  }
}

/// What a call answers: its text, and the image of a screenshot for an agent that is shown images.
struct Answer {
  text: String,
  image: Option<AnswerImage>,
}

impl From<String> for Answer {
  fn from(text: String) -> Self {
    Self { text, image: None }
  }
}

/// The browser tools of one invocation.
pub(in crate::execution) struct StepBrowser {
  browser: Arc<ProjectBrowser>,
  run: RunId,
  permissions: AgentPermissions,
  worktree: PathBuf,
  /// Screenshots come back as an image too: the agent is Claude Code, which shows MCP image blocks.
  images: bool,
}

impl StepBrowser {
  /// The browser tools of a step of `run` with `permissions`, working in `worktree`.
  pub(in crate::execution) const fn new(
    browser: Arc<ProjectBrowser>,
    run: RunId,
    permissions: AgentPermissions,
    worktree: PathBuf,
  ) -> Self {
    Self {
      browser,
      run,
      permissions,
      worktree,
      images: false,
    }
  }

  /// These tools, a screenshot answered with the image as well.
  #[must_use]
  pub(in crate::execution) const fn with_images(mut self) -> Self {
    self.images = true;
    self
  }

  /// Whether `call` asks a person first, and the site its approval names; why it is refused, when the tab it acts on
  /// is not the run's or the run has none.
  pub(in crate::execution::step) async fn look(&self, call: &BrowserCall) -> Result<Looked, String> {
    let tool = call.tool();
    if tool.reach() == BrowserReach::Reads {
      return Ok(Looked {
        asks: false,
        target: None,
      });
    }
    let site = match call.opened_url() {
      Some(url) => Some(url.host().clone()),
      None => self.page_host(call.tab()).await.map_err(|error| error.to_string())?,
    };
    let policy = browser::decide(tool, site.as_ref(), &self.permissions)
      .ok_or_else(|| format!("`{}` is not available in this step", tool.as_str()))?;
    Ok(Looked {
      asks: policy == ToolPolicy::Asks,
      target: site.map(|host| host.to_string()),
    })
  }

  /// The host of the page in `tab` (the current tab for `None`); `None` for a page without a web host.
  async fn page_host(&self, tab: Option<&TabId>) -> Result<Option<SiteHost>, ToolError> {
    let info = self.tab(tab).await?.info().await?;
    Ok(PageUrl::parse(&info.url).ok().map(|url| url.host().clone()))
  }

  /// Runs `call`, approved or not needing an approval.
  pub(in crate::execution::step) async fn run(&self, call: BrowserCall) -> ToolResult {
    match self.answer(call).await {
      Ok(Answer {
        text,
        image: Some(image),
      }) => ToolResult::with_image(text, image),
      Ok(Answer { text, image: None }) => ToolResult::shown(text),
      Err(error) => ToolResult::unavailable(&error.to_string()),
    }
  }

  async fn answer(&self, call: BrowserCall) -> Result<Answer, ToolError> {
    let tab = call.tab().cloned();
    let tab = tab.as_ref();
    match call {
      BrowserCall::Tabs => self.list().await.map(Answer::from),
      BrowserCall::Open { url, new_tab } => self.open(&url, new_tab).await.map(Answer::from),
      BrowserCall::Close { .. } => self.close(tab).await.map(Answer::from),
      BrowserCall::Navigate { to, .. } => self.on(tab, async |tab| navigation::navigate(tab, to).await).await,
      BrowserCall::ReadPage {
        query,
        interactive_only,
        max_chars,
        ..
      } => {
        self
          .on(tab, async |tab| {
            page::read(tab, query.as_deref(), interactive_only, max_chars).await
          })
          .await
      }
      BrowserCall::PageText { max_chars, .. } => self.on(tab, async |tab| page::text(tab, max_chars).await).await,
      BrowserCall::Click {
        target, button, count, ..
      } => {
        self
          .on(tab, async |tab| act::click(tab, &target, button, count).await)
          .await
      }
      BrowserCall::Type {
        element, text, submit, ..
      } => {
        self
          .on(tab, async |tab| act::type_text(tab, &element, &text, submit).await)
          .await
      }
      BrowserCall::Key { keys, .. } => self.on(tab, async |tab| keys::press(tab, &keys).await).await,
      BrowserCall::Scroll { target, .. } => self.on(tab, async |tab| act::scroll(tab, &target).await).await,
      BrowserCall::Screenshot { element, full_page, .. } => {
        let saved = (self.worktree.as_path(), self.images);
        self
          .on(tab, async |tab| {
            shot::take(tab, element.as_ref(), full_page, saved).await
          })
          .await
      }
      BrowserCall::Console {
        errors_only,
        pattern,
        limit,
        ..
      } => {
        self
          .on(tab, async |tab| {
            Ok(logs::console(tab, errors_only, pattern.as_deref(), limit))
          })
          .await
      }
      BrowserCall::Network { url_pattern, limit, .. } => {
        self
          .on(tab, async |tab| Ok(logs::network(tab, url_pattern.as_deref(), limit)))
          .await
      }
      BrowserCall::Wait {
        until, timeout_seconds, ..
      } => {
        self
          .on(tab, async |tab| script::wait(tab, &until, timeout_seconds).await)
          .await
      }
      BrowserCall::Eval { expression, .. } => self.on(tab, async |tab| script::eval(tab, &expression).await).await,
    }
  }

  /// What `act` answers on the run's tab `tab` (its current one for `None`), followed by the dialogs the page opened
  /// meanwhile, which were dismissed.
  async fn on<A: Into<Answer>>(
    &self,
    tab: Option<&TabId>,
    act: impl AsyncFnOnce(&Tab) -> Result<A, ToolError>,
  ) -> Result<Answer, ToolError> {
    let tab = self.tab(tab).await?;
    let mut answer: Answer = act(&tab).await?.into();
    let dialogs = tab.log().take_dialogs();
    for dialog in dialogs {
      // Writing to a String cannot fail.
      let _ = write!(answer.text, "\n{PAGE_DATA_NOTICE}\nThe page opened a dialog: {dialog}.");
    }
    Ok(answer)
  }

  /// The run's tab `tab`, or its current one for `None`.
  async fn tab(&self, tab: Option<&TabId>) -> Result<Tab, BrowserError> {
    let target = tab.map(|tab| TargetId::from(tab.as_str()));
    self.browser.tab(self.run, target.as_ref()).await
  }
}
