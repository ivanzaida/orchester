//! `browser_tabs`, `browser_open`, `browser_close` and `browser_navigate`: the run's tabs and the pages they hold.

use std::fmt::Write as _;

use orchester_core::orchester_tools::browser::{HistoryStep, PAGE_DATA_NOTICE, PageUrl, TabId};
use serde_json::{Value, json};
use tokio::{sync::mpsc, time::Instant};

use super::{StepBrowser, ToolError};
use crate::browser::{
  LOAD_LIMIT, Tab,
  cdp::{CdpError, CdpEvent},
};

impl StepBrowser {
  /// The run's tabs, the current one marked.
  pub(super) async fn list(&self) -> Result<String, ToolError> {
    let tabs = self.browser.tabs(self.run).await?;
    if tabs.is_empty() {
      return Ok("This run has no open tab: open a page with browser_open.".to_owned());
    }
    let current = self.browser.current(self.run).await;
    let mut text = format!("{PAGE_DATA_NOTICE}\nThis run's tabs:");
    for tab in tabs {
      let mark = if current.as_ref() == Some(&tab.id) {
        " (current)"
      } else {
        ""
      };
      // Writing to a String cannot fail.
      let _ = write!(text, "\n- {}{mark}: \"{}\" {}", tab.id, tab.title, tab.url);
    }
    Ok(text)
  }

  /// Opens `url` in a new tab, or in the current one unless `new_tab` or the run has none, and waits for its load.
  pub(super) async fn open(&self, url: &PageUrl, new_tab: bool) -> Result<String, ToolError> {
    let current = if new_tab {
      None
    } else {
      self.browser.current(self.run).await
    };
    let tab = match current {
      Some(current) => {
        let tab = self.browser.tab(self.run, Some(&current)).await?;
        self.browser.load(self.run, &tab, url.as_str()).await?;
        tab
      }
      None => {
        let opened = self.browser.open_tab(self.run, url.as_str()).await?;
        self.browser.tab(self.run, Some(&opened)).await?
      }
    };
    let info = tab.info().await?;
    Ok(format!(
      "{PAGE_DATA_NOTICE}\nOpened in tab {}: \"{}\" {}",
      info.id, info.title, info.url
    ))
  }

  /// Closes the run's tab `tab`, or its current one for `None`.
  pub(super) async fn close(&self, tab: Option<&TabId>) -> Result<String, ToolError> {
    let closing = self.tab(tab).await?;
    self.browser.close_tab(self.run, closing.id()).await?;
    Ok(match self.browser.current(self.run).await {
      Some(current) => format!("Closed tab {}. The current tab is {current}.", closing.id()),
      None => format!("Closed tab {}. This run has no open tab now.", closing.id()),
    })
  }
}

/// Goes back or forward in `tab`'s history, or reloads it, and waits until the page it shows has loaded.
pub(super) async fn navigate(tab: &Tab, to: HistoryStep) -> Result<String, ToolError> {
  let mut events = tab.events();
  match to {
    HistoryStep::Reload => {
      tab.call("Page.reload", json!({})).await?;
    }
    HistoryStep::Back | HistoryStep::Forward => {
      let history = tab.call("Page.getNavigationHistory", json!({})).await?;
      let entry = history_entry(&history, to)?;
      tab
        .call("Page.navigateToHistoryEntry", json!({ "entryId": entry }))
        .await?;
    }
  }
  shown(&mut events).await?;
  let info = tab.info().await?;
  Ok(format!(
    "{PAGE_DATA_NOTICE}\nTab {} shows \"{}\" {}",
    info.id, info.title, info.url
  ))
}

/// The id of the entry one step `to` from the current one of `history` (`Page.getNavigationHistory`).
fn history_entry(history: &Value, to: HistoryStep) -> Result<i64, ToolError> {
  let entries = history["entries"].as_array().map_or(&[][..], Vec::as_slice);
  let current = history["currentIndex"].as_i64().unwrap_or_default();
  let (index, missing) = match to {
    HistoryStep::Forward => (current + 1, "there is no page to go forward to in this tab"),
    _ => (current - 1, "there is no page to go back to in this tab"),
  };
  usize::try_from(index)
    .ok()
    .and_then(|index| entries.get(index))
    .and_then(|entry| entry["id"].as_i64())
    .ok_or_else(|| ToolError::Page(missing.to_owned()))
}

/// Waits up to [`LOAD_LIMIT`] until the tab shows the page it went to: loaded, moved within its document, or restored
/// from the back-forward cache, which loads nothing.
async fn shown(events: &mut mpsc::UnboundedReceiver<CdpEvent>) -> Result<(), ToolError> {
  let deadline = Instant::now() + LOAD_LIMIT;
  loop {
    let event = match tokio::time::timeout_at(deadline, events.recv()).await {
      Ok(Some(event)) => event,
      Ok(None) => return Err(CdpError::Closed.into()),
      Err(_) => {
        return Err(
          CdpError::NoEvent {
            event: "Page.loadEventFired".to_owned(),
            limit: LOAD_LIMIT,
          }
          .into(),
        );
      }
    };
    let restored = event.method == "Page.frameNavigated" && event.params["type"] == "BackForwardCacheRestore";
    if restored || event.method == "Page.loadEventFired" || event.method == "Page.navigatedWithinDocument" {
      return Ok(());
    }
  }
}
