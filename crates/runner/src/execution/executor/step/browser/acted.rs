//! What the browser tools tell the person's *Browser* window an agent did, for *Agent is acting*
//! ([`crate::browser::ProjectBrowser::agent_acted`]): the tool, a short summary (*typed 3 characters in Name*) and the
//! box of the element it acted on. Only the calls that go to a page or act on one report it, once carried out; the
//! reading tools and `browser_close` do not.

use orchester_core::orchester_tools::browser::{
  BrowserCall, ClickTarget, ElementRef, HistoryStep, KeyPress, ScrollDirection, ScrollTarget,
};
use orchester_ipc::browser::{AgentAction, ElementBox};
use serde_json::json;

use super::act;
use crate::browser::Tab;

/// An acting call, before it is carried out.
pub(super) struct Acted {
  tool: &'static str,
  /// What it does, before the name of the element it acts on when it acts on one (*typed 3 characters in*).
  did: String,
  element: Option<ElementRef>,
}

impl Acted {
  /// What `call` does, when it is a call that acts.
  pub(super) fn of(call: &BrowserCall) -> Option<Self> {
    let (did, element) = match call {
      BrowserCall::Open { url, .. } => (format!("opened {}", url.as_str()), None),
      BrowserCall::Navigate { to, .. } => {
        let did = match to {
          HistoryStep::Back => "went back",
          HistoryStep::Forward => "went forward",
          HistoryStep::Reload => "reloaded the page",
        };
        (did.to_owned(), None)
      }
      BrowserCall::Click { target, count, .. } => {
        let clicked = if *count == 2 { "double-clicked" } else { "clicked" };
        match target {
          ClickTarget::Element(element) => (clicked.to_owned(), Some(element.clone())),
          ClickTarget::Point { x, y } => (format!("{clicked} at ({x}, {y})"), None),
        }
      }
      BrowserCall::Type { element, text, .. } => (
        format!("typed {} characters in", text.chars().count()),
        Some(element.clone()),
      ),
      BrowserCall::Key { keys, .. } => {
        let keys: Vec<String> = keys.iter().map(key_name).collect();
        (format!("pressed {}", keys.join(", ")), None)
      }
      BrowserCall::Scroll { target, .. } => match target {
        ScrollTarget::Element(element) => ("scrolled to".to_owned(), Some(element.clone())),
        ScrollTarget::Page { direction, .. } => (format!("scrolled {}", direction_name(*direction)), None),
      },
      BrowserCall::Eval { .. } => ("ran JavaScript".to_owned(), None),
      BrowserCall::Tabs
      | BrowserCall::Close { .. }
      | BrowserCall::ReadPage { .. }
      | BrowserCall::PageText { .. }
      | BrowserCall::Screenshot { .. }
      | BrowserCall::Console { .. }
      | BrowserCall::Network { .. }
      | BrowserCall::Wait { .. } => return None,
    };
    Some(Self {
      tool: call.tool().as_str(),
      did,
      element,
    })
  }

  /// The action as the window gets it, once carried out on `tab`: the element's accessible name and box as the page
  /// has them now. A name or box the page no longer gives, as after a click that loaded another page, becomes the
  /// element's ref and no box: the action happened and is reported all the same.
  pub(super) async fn on(self, tab: &Tab) -> AgentAction {
    let Some(element) = self.element else {
      return AgentAction {
        tool: self.tool.to_owned(),
        summary: self.did,
        element: None,
      };
    };
    let (name, element_box) = match tab.node(&element) {
      Ok(node) => (name_of(tab, node).await, box_of(tab, node).await),
      Err(_) => (None, None),
    };
    let name = name.unwrap_or_else(|| element.as_str().to_owned());
    AgentAction {
      tool: self.tool.to_owned(),
      summary: format!("{} {name}", self.did),
      element: element_box,
    }
  }
}

/// The accessible name of `node` (a `backendNodeId`), as `browser_read_page` shows it; `None` when it has none.
async fn name_of(tab: &Tab, node: i64) -> Option<String> {
  let tree = tab
    .call(
      "Accessibility.getPartialAXTree",
      json!({ "backendNodeId": node, "fetchRelatives": false }),
    )
    .await
    .ok()?;
  let name = tree["nodes"][0]["name"]["value"].as_str()?.trim();
  (!name.is_empty()).then(|| name.to_owned())
}

/// The box of `node` in the visible page, in CSS pixels, the corners' bounds; `None` when it has none.
async fn box_of(tab: &Tab, node: i64) -> Option<ElementBox> {
  let quad = act::quad(tab, node).await.ok()??;
  let xs = [quad[0], quad[2], quad[4], quad[6]];
  let ys = [quad[1], quad[3], quad[5], quad[7]];
  let (left, right) = (
    xs.into_iter().fold(f64::MAX, f64::min),
    xs.into_iter().fold(f64::MIN, f64::max),
  );
  let (top, bottom) = (
    ys.into_iter().fold(f64::MAX, f64::min),
    ys.into_iter().fold(f64::MIN, f64::max),
  );
  Some(ElementBox {
    x: left.round() as i32,
    y: top.round() as i32,
    width: (right - left).round() as u32,
    height: (bottom - top).round() as u32,
  })
}

/// `key` as the call named it: its modifiers and key joined by `+` (`Ctrl+A`).
fn key_name(key: &KeyPress) -> String {
  let modifiers = key.modifiers().iter().map(|modifier| format!("{modifier:?}+"));
  modifiers.chain([key.key().to_owned()]).collect()
}

const fn direction_name(direction: ScrollDirection) -> &'static str {
  match direction {
    ScrollDirection::Up => "up",
    ScrollDirection::Down => "down",
    ScrollDirection::Left => "left",
    ScrollDirection::Right => "right",
  }
}
