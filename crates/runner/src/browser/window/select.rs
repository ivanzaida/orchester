//! `<select>` drop-down lists, which a headless frame does not show (`docs/design/browser.md`, "What frames do not
//! show"): a press on one is found by a hit test and its options read for the window to draw; the person's choice is
//! set on the element with the events a page listens to.

use orchester_ipc::browser::{SelectId, SelectList};
use serde_json::json;

use super::hit;
use crate::browser::{
  BrowserError,
  cdp::{Cdp, SessionId},
};

/// Reads a single-choice `<select>` that the person may open, focusing it, as a [`SelectList`] named `select`; `null`
/// for any other element.
const READ_LIST: &str = "function(select) {
  if (this.tagName !== 'SELECT' || this.multiple || this.size > 1 || this.disabled) return null;
  this.focus();
  const box = this.getBoundingClientRect();
  return {
    select,
    options: Array.from(this.options, (option) => ({
      label: option.label, selected: option.selected, disabled: option.disabled,
    })),
    element: {
      x: Math.round(box.left), y: Math.round(box.top),
      width: Math.max(0, Math.round(box.width)), height: Math.max(0, Math.round(box.height)),
    },
  };
}";

/// Chooses option `index` of the `<select>` as the person's own choice would; `false` for no such enabled option.
const CHOOSE: &str = "function(index) {
  if (this.tagName !== 'SELECT' || index >= this.options.length || this.options[index].disabled) return false;
  this.selectedIndex = index;
  this.dispatchEvent(new Event('input', { bubbles: true }));
  this.dispatchEvent(new Event('change', { bubbles: true }));
  return true;
}";

/// The `<select>` list at `x`, `y` of the tab of `session`, in CSS pixels; `None` when another element, or none, is
/// there, and the press goes to the page as it is.
pub(super) async fn at(cdp: &Cdp, session: &SessionId, x: i32, y: i32) -> Result<Option<SelectList>, BrowserError> {
  let Some(node) = hit::node_at(cdp, session, x, y).await? else {
    return Ok(None);
  };
  let read = hit::call_on(cdp, session, node, READ_LIST, &[json!({ "value": node })]).await?;
  if read.is_null() {
    return Ok(None);
  }
  serde_json::from_value(read)
    .map(Some)
    .map_err(|error| BrowserError::Element(format!("the list's options cannot be read: {error}")))
}

/// Chooses option `index` of the list `select` in the tab of `session`.
pub(super) async fn choose(cdp: &Cdp, session: &SessionId, select: SelectId, index: u32) -> Result<(), BrowserError> {
  let chosen = hit::call_on(cdp, session, select.get(), CHOOSE, &[json!({ "value": index })]).await?;
  if chosen == json!(true) {
    Ok(())
  } else {
    Err(BrowserError::Element(format!(
      "the list has no option {index} to choose"
    )))
  }
}
