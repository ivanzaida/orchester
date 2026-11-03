//! `browser_click`, `browser_type` and `browser_scroll`: input as the browser gets a person's, on an element named by a
//! ref of the tab's last read or on a point of the visible page.

use orchester_core::orchester_tools::browser::{
  ClickTarget, ElementRef, KeyPress, MouseButton, ScrollDirection, ScrollTarget,
};
use serde_json::{Value, json};

use super::{ToolError, keys};
use crate::browser::Tab;

/// Selects what a field holds, so that typing replaces it: its text, or a content-editable element's content.
const SELECT_ALL: &str = "function() { this.focus(); if (typeof this.select === 'function') { this.select(); } else { \
                          const range = document.createRange(); range.selectNodeContents(this); const selection = \
                          window.getSelection(); selection.removeAllRanges(); selection.addRange(range); } }";

/// Clicks `target` with `button`, `count` times in a row (2 for a double click).
pub(super) async fn click(
  tab: &Tab,
  target: &ClickTarget,
  button: MouseButton,
  count: u8,
) -> Result<String, ToolError> {
  let ((x, y), what) = match target {
    ClickTarget::Element(element) => (centre(tab, element).await?, element.as_str().to_owned()),
    ClickTarget::Point { x, y } => ((f64::from(*x), f64::from(*y)), format!("({x}, {y})")),
  };
  let button = match button {
    MouseButton::Left => "left",
    MouseButton::Right => "right",
    MouseButton::Middle => "middle",
  };
  mouse(tab, json!({ "type": "mouseMoved", "x": x, "y": y })).await?;
  for clicks in 1..=count {
    for kind in ["mousePressed", "mouseReleased"] {
      mouse(
        tab,
        json!({ "type": kind, "x": x, "y": y, "button": button, "clickCount": clicks }),
      )
      .await?;
    }
  }
  let times = if count == 1 {
    String::new()
  } else {
    format!(" {count} times")
  };
  Ok(format!(
    "Clicked {what} with the {button} button{times}. Read the page again to see what changed."
  ))
}

/// Replaces the value of the field `element` with `text`, then presses Enter when `submit`.
pub(super) async fn type_text(tab: &Tab, element: &ElementRef, text: &str, submit: bool) -> Result<String, ToolError> {
  let node = tab.node(element)?;
  tab
    .call("DOM.scrollIntoViewIfNeeded", json!({ "backendNodeId": node }))
    .await?;
  let resolved = tab.call("DOM.resolveNode", json!({ "backendNodeId": node })).await?;
  let object = resolved["object"]["objectId"]
    .as_str()
    .ok_or_else(|| ToolError::Page(format!("{} is no longer in the page", element.as_str())))?;
  tab
    .call(
      "Runtime.callFunctionOn",
      json!({ "objectId": object, "functionDeclaration": SELECT_ALL }),
    )
    .await?;
  if text.is_empty() {
    keys::press(tab, &[key("Delete")?]).await?;
  } else {
    tab.call("Input.insertText", json!({ "text": text })).await?;
  }
  if submit {
    keys::press(tab, &[key("Enter")?]).await?;
  }
  let then = if submit { ", then pressed Enter" } else { "" };
  Ok(format!(
    "Typed {} characters into {}{then}.",
    text.chars().count(),
    element.as_str()
  ))
}

/// Scrolls `target` into view, or the page by its pixels (a screen when none).
pub(super) async fn scroll(tab: &Tab, target: &ScrollTarget) -> Result<String, ToolError> {
  match target {
    ScrollTarget::Element(element) => {
      let node = tab.node(element)?;
      tab
        .call("DOM.scrollIntoViewIfNeeded", json!({ "backendNodeId": node }))
        .await?;
      Ok(format!("Scrolled {} into view.", element.as_str()))
    }
    ScrollTarget::Page { direction, pixels } => {
      let screen = match direction {
        ScrollDirection::Up | ScrollDirection::Down => "innerHeight",
        ScrollDirection::Left | ScrollDirection::Right => "innerWidth",
      };
      let distance = pixels.map_or_else(|| format!("window.{screen}"), |pixels| pixels.to_string());
      let (left, top) = match direction {
        ScrollDirection::Up => ("0".to_owned(), format!("-({distance})")),
        ScrollDirection::Down => ("0".to_owned(), distance),
        ScrollDirection::Left => (format!("-({distance})"), "0".to_owned()),
        ScrollDirection::Right => (distance, "0".to_owned()),
      };
      let expression = format!(
        "window.scrollBy({{ left: {left}, top: {top}, behavior: 'instant' }}); \
         [Math.round(window.scrollX), Math.round(window.scrollY)]"
      );
      let answer = tab
        .call(
          "Runtime.evaluate",
          json!({ "expression": expression, "returnByValue": true }),
        )
        .await?;
      let at = &answer["result"]["value"];
      Ok(format!("Scrolled the page to x {}, y {}.", at[0], at[1]))
    }
  }
}

/// The centre of `element` in the visible page, once scrolled into view.
pub(super) async fn centre(tab: &Tab, element: &ElementRef) -> Result<(f64, f64), ToolError> {
  let node = tab.node(element)?;
  tab
    .call("DOM.scrollIntoViewIfNeeded", json!({ "backendNodeId": node }))
    .await?;
  let quad = quad(tab, node).await?.ok_or_else(|| {
    ToolError::Page(format!(
      "{} has no box on the page: it is hidden or has no size",
      element.as_str()
    ))
  })?;
  let x = (quad[0] + quad[2] + quad[4] + quad[6]) / 4.0;
  let y = (quad[1] + quad[3] + quad[5] + quad[7]) / 4.0;
  Ok((x, y))
}

/// The corners of the first box of `node` (a `backendNodeId`) in the visible page, `x` and `y` of each in turn; `None`
/// when it has none: it is hidden or has no size.
pub(super) async fn quad(tab: &Tab, node: i64) -> Result<Option<[f64; 8]>, ToolError> {
  let quads = tab
    .call("DOM.getContentQuads", json!({ "backendNodeId": node }))
    .await?;
  let quad: Vec<f64> = quads["quads"][0]
    .as_array()
    .map(|points| points.iter().filter_map(Value::as_f64).collect())
    .unwrap_or_default();
  Ok(quad.try_into().ok())
}

async fn mouse(tab: &Tab, event: Value) -> Result<(), ToolError> {
  tab.call("Input.dispatchMouseEvent", event).await?;
  Ok(())
}

fn key(name: &str) -> Result<KeyPress, ToolError> {
  KeyPress::new(name).map_err(ToolError::Page)
}
