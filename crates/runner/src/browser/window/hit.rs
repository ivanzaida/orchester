//! The element under a point of a page, found by a hit test, and calling a function on it: for what a headless frame
//! does not show of it (`docs/design/browser.md`, "What frames do not show").

use serde_json::{Value, json};

use crate::browser::{
  BrowserError,
  cdp::{Cdp, CdpError, SessionId},
};

/// The browser's id of the node at `x`, `y` of the tab of `session`, in CSS pixels; `None` when none is there.
pub(super) async fn node_at(cdp: &Cdp, session: &SessionId, x: i32, y: i32) -> Result<Option<u64>, BrowserError> {
  let hit = cdp
    .call(
      Some(session),
      "DOM.getNodeForLocation",
      json!({ "x": x, "y": y, "includeUserAgentShadowDOM": false }),
    )
    .await;
  match hit {
    Ok(node) => Ok(node.get("backendNodeId").and_then(Value::as_u64)),
    // Nothing is there, or the page is between documents.
    Err(CdpError::Refused { .. }) => Ok(None),
    Err(error) => Err(error.into()),
  }
}

/// Calls `function` on the page's node `node` with `arguments` and returns its value. The page's handle on the node is
/// let go of again, as it would keep the node alive until the page goes, and a hit test comes many times a minute.
pub(super) async fn call_on(
  cdp: &Cdp,
  session: &SessionId,
  node: u64,
  function: &str,
  arguments: &[Value],
) -> Result<Value, BrowserError> {
  let resolved = cdp
    .call(Some(session), "DOM.resolveNode", json!({ "backendNodeId": node }))
    .await?;
  let object = resolved
    .pointer("/object/objectId")
    .and_then(Value::as_str)
    .ok_or_else(|| BrowserError::Element("the element is no longer in the page".to_owned()))?;
  let called = cdp
    .call(
      Some(session),
      "Runtime.callFunctionOn",
      json!({
        "objectId": object,
        "functionDeclaration": function,
        "arguments": arguments,
        "returnByValue": true,
      }),
    )
    .await;
  if let Err(error) = cdp
    .call(Some(session), "Runtime.releaseObject", json!({ "objectId": object }))
    .await
  {
    tracing::debug!(%error, "the page's handle on an element was not let go of");
  }
  let called = called?;
  if let Some(exception) = called.get("exceptionDetails") {
    return Err(BrowserError::Element(format!("the page threw: {exception}")));
  }
  Ok(called.pointer("/result/value").cloned().unwrap_or(Value::Null))
}
