//! `ui_act` and `ui_screenshot` of the browser driver: input as the browser gets a person's, inside the tab only, on
//! the element a snapshot's ref names; a screenshot of the page, or of one element, saved in the run's storage.

use std::path::Path;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use orchester_core::orchester_tools::{
  browser::KeyPress,
  ui::{UiAction, UiRef},
};
use serde_json::{Value, json};

use super::{
  super::super::{
    browser::{ToolError, act, keys, shot},
    run_app::save_png_bytes,
  },
  snapshot::node_of,
};
use crate::browser::{BrowserError, Tab, cdp::CdpError};

/// Chooses the option whose label, text or value is the argument in a `<select>`, or sets a field's value, with the
/// events a person's choice fires; answers why not, or `null` once done.
const SELECT: &str = "function(choice) { if (this instanceof HTMLSelectElement) { const option = \
                      Array.from(this.options).find(o => o.label === choice || o.text.trim() === choice || o.value === \
                      choice); if (!option) { return 'it has no option ' + JSON.stringify(choice) + ': its options are ' \
                      + Array.from(this.options).map(o => JSON.stringify(o.label)).join(', '); } this.value = \
                      option.value; } else if ('value' in this && !this.readOnly && !this.disabled) { this.focus(); \
                      this.value = choice; } else { return 'unsupported for this element: it is no list and no field'; \
                      } this.dispatchEvent(new Event('input', { bubbles: true })); this.dispatchEvent(new \
                      Event('change', { bubbles: true })); return null; }";

/// The DOM node `reference` names, still in the page; why not.
async fn node(tab: &Tab, reference: &UiRef) -> Result<i64, String> {
  let named = reference.as_str();
  let node =
    node_of(named).ok_or_else(|| format!("`{named}` is not a ref of this page's snapshot: take a snapshot"))?;
  match tab.call("DOM.resolveNode", json!({ "backendNodeId": node })).await {
    Ok(_) => Ok(node),
    Err(CdpError::Refused { .. }) => Err(stale(named)),
    Err(error) => Err(error.to_string()),
  }
}

fn stale(reference: &str) -> String {
  format!("{reference} is no longer in the page: take a new snapshot and use a ref it gives")
}

/// What the browser tools' helpers said, a node gone meanwhile said as a stale ref.
fn failed(reference: &str, error: ToolError) -> String {
  match error {
    ToolError::Browser(BrowserError::Cdp(CdpError::Refused { .. })) => stale(reference),
    error => error.to_string(),
  }
}

/// Carries out `action` on `target`; a key without a target goes to the focused element.
pub(super) async fn act(tab: &Tab, target: Option<&UiRef>, action: &UiAction) -> Result<(), String> {
  let Some(target) = target else {
    let UiAction::Key(key) = action else {
      return Err(format!(
        "{} needs the ref of an element from ui_snapshot",
        action.as_str()
      ));
    };
    return press(tab, key).await;
  };
  let node = node(tab, target).await?;
  let label = target.as_str();
  match action {
    UiAction::Click => {
      let point = act::centre_of(tab, node, label)
        .await
        .map_err(|error| failed(label, error))?;
      act::click_at(tab, point, "left", 1)
        .await
        .map_err(|error| failed(label, error))
    }
    UiAction::Type(text) => act::fill(tab, node, label, text)
      .await
      .map_err(|error| failed(label, error)),
    UiAction::Key(key) => {
      call(tab, label, "DOM.focus", json!({ "backendNodeId": node })).await?;
      press(tab, key).await
    }
    UiAction::Scroll => call(
      tab,
      label,
      "DOM.scrollIntoViewIfNeeded",
      json!({ "backendNodeId": node }),
    )
    .await
    .map(drop),
    UiAction::Select(choice) => select(tab, node, label, choice).await,
  }
}

/// Presses `key`: one character, a key's name such as `Enter`, or one with modifiers such as `Ctrl+A`.
async fn press(tab: &Tab, key: &str) -> Result<(), String> {
  let key = KeyPress::new(key)?;
  keys::press(tab, &[key])
    .await
    .map(drop)
    .map_err(|error| error.to_string())
}

async fn call(tab: &Tab, label: &str, method: &str, params: Value) -> Result<Value, String> {
  tab.call(method, params).await.map_err(|error| match error {
    CdpError::Refused { .. } => stale(label),
    error => error.to_string(),
  })
}

/// Chooses `choice` in the list `node`, or sets the field `node` to it.
async fn select(tab: &Tab, node: i64, label: &str, choice: &str) -> Result<(), String> {
  let resolved = call(tab, label, "DOM.resolveNode", json!({ "backendNodeId": node })).await?;
  let object = resolved["object"]["objectId"].as_str().ok_or_else(|| stale(label))?;
  let answer = call(
    tab,
    label,
    "Runtime.callFunctionOn",
    json!({
      "objectId": object,
      "functionDeclaration": SELECT,
      "arguments": [{ "value": choice }],
      "returnByValue": true,
    }),
  )
  .await?;
  match answer["result"]["value"].as_str() {
    Some(why) => Err(format!("{label}: {why}")),
    None => Ok(()),
  }
}

/// A PNG of the visible page, or of the element `target` only, saved in the run's storage `artifacts`; its path.
pub(super) async fn screenshot(tab: &Tab, target: Option<&UiRef>, artifacts: &Path) -> Result<String, String> {
  let mut params = json!({ "format": "png" });
  if let Some(target) = target {
    let node = node(tab, target).await?;
    let label = target.as_str();
    params["clip"] = shot::node_clip(tab, node, label)
      .await
      .map_err(|error| failed(label, error))?;
    params["captureBeyondViewport"] = Value::Bool(true);
  }
  let taken = tab
    .call_within("Page.captureScreenshot", params, shot::SHOT_LIMIT)
    .await
    .map_err(|error| error.to_string())?;
  let png = taken["data"]
    .as_str()
    .and_then(|data| STANDARD.decode(data).ok())
    .ok_or_else(|| "the browser's screenshot is not a PNG".to_owned())?;
  save_png_bytes(artifacts, &png)
}
