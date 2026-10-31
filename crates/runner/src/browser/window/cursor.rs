//! The mouse cursor's shape over a page (`docs/design/browser.md`, "What frames do not show"), which CDP does not
//! report: found from the element under the pointer, as its computed `cursor` or what it is.

use orchester_ipc::browser::PageCursor;

use super::hit;
use crate::browser::{
  BrowserError,
  cdp::{Cdp, CdpError, SessionId},
};

/// A [`PageCursor`] by its name. A page's `pointer` and `text` count first. What `auto` or `default` leaves open is
/// told from the element or what holds it, across shadow roots: browsers give a `<button>` the `default` cursor, and a
/// `contenteditable` the `auto` one. Any other cursor of a page (`grab`, `not-allowed`) is drawn as the arrow.
const SHAPE: &str = "function() {
  const start = this.nodeType === Node.ELEMENT_NODE ? this : this.parentElement;
  if (!start) return 'arrow';
  const cursor = getComputedStyle(start).cursor;
  if (cursor === 'pointer') return 'hand';
  if (cursor === 'text' || cursor === 'vertical-text') return 'text';
  if (cursor !== 'auto' && cursor !== 'default') return 'arrow';
  const presses = 'a[href], area[href], button, summary, [role=button], [role=link], '
    + 'input[type=button], input[type=submit], input[type=reset], input[type=image]';
  const unlike = ['button', 'submit', 'reset', 'image', 'checkbox', 'radio', 'file', 'range', 'color', 'hidden'];
  for (let element = start; element; element = element.parentElement || element.getRootNode().host) {
    if (element.matches(':disabled')) return 'arrow';
    if (element.matches(presses)) return 'hand';
    if (element.isContentEditable || element.tagName === 'TEXTAREA') return 'text';
    if (element.tagName === 'INPUT' && !unlike.includes(element.type)) return 'text';
  }
  return 'arrow';
}";

/// The cursor's shape at `x`, `y` of the tab of `session`, in CSS pixels. The arrow when no element is there any more,
/// or the page cannot tell, between two documents; a lost connection is an error.
pub(super) async fn at(cdp: &Cdp, session: &SessionId, x: i32, y: i32) -> Result<PageCursor, BrowserError> {
  let Some(node) = hit::node_at(cdp, session, x, y).await? else {
    return Ok(PageCursor::Arrow);
  };
  match hit::call_on(cdp, session, node, SHAPE, &[]).await {
    // Only the names `SHAPE` gives come back; anything else is the arrow as well.
    Ok(name) => Ok(serde_json::from_value(name).unwrap_or_default()),
    Err(BrowserError::Element(_) | BrowserError::Cdp(CdpError::Refused { .. })) => Ok(PageCursor::Arrow),
    Err(error) => Err(error),
  }
}
