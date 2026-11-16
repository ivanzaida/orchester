//! `ui_wait` of the browser driver: the page looked at again until it holds the condition, within the call's time.
//! `idle` is a page whose document finished loading and that drew two frames since; `element` an element of its
//! snapshot whose name, role or value holds the query; `selector` an element a CSS selector matches, an invalid
//! selector failing at once; `text` its text holding the query; `url` its address holding it.

use std::time::Duration;

use orchester_core::orchester_tools::ui::UiCondition;
use serde_json::{Value, json};
use tokio::time::Instant;

use super::snapshot::Snapshot;
use crate::browser::{Tab, cdp::CdpError};

/// How often a wait looks at the page again.
const POLL: Duration = Duration::from_millis(200);
/// Resolves once the page drew two frames: what it had to draw is drawn.
const FRAMES: &str = "new Promise(done => requestAnimationFrame(() => requestAnimationFrame(() => done(true))))";

/// What a look at the page found.
enum Look {
  /// The condition holds: the answer.
  Met(Value),
  NotYet,
}

/// Waits until `tab`'s page holds `condition`, at most `timeout_ms`.
pub(super) async fn wait(tab: &Tab, condition: &UiCondition, timeout_ms: u32) -> Result<Value, String> {
  let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_ms));
  loop {
    if let Look::Met(answer) = look(tab, condition).await? {
      return Ok(answer);
    }
    if Instant::now() + POLL > deadline {
      return Err(match condition {
        UiCondition::Idle => format!("the page did not finish loading within {timeout_ms} ms"),
        UiCondition::Element(query) => format!("no element holding `{query}` was shown within {timeout_ms} ms"),
        UiCondition::Selector(query) => format!("no element matched `{query}` within {timeout_ms} ms"),
        UiCondition::Text(query) => format!("the page's text did not hold `{query}` within {timeout_ms} ms"),
        UiCondition::Url(query) => format!("the page's address did not hold `{query}` within {timeout_ms} ms"),
      });
    }
    tokio::time::sleep(POLL).await;
  }
}

async fn look(tab: &Tab, condition: &UiCondition) -> Result<Look, String> {
  let met = |found: bool, answer: Value| if found { Look::Met(answer) } else { Look::NotYet };
  Ok(match condition {
    UiCondition::Idle => {
      if evaluate(tab, "document.readyState").await? != Some(json!("complete")) {
        return Ok(Look::NotYet);
      }
      met(evaluate(tab, FRAMES).await?.is_some(), json!({"met": "idle"}))
    }
    UiCondition::Element(query) => {
      let tree = match tab.call("Accessibility.getFullAXTree", json!({})).await {
        Ok(tree) => tree,
        Err(CdpError::Refused { .. }) => return Ok(Look::NotYet),
        Err(error) => return Err(error.to_string()),
      };
      let nodes = tree["nodes"].as_array().map_or(&[][..], Vec::as_slice);
      let (matches, _) = Snapshot::of(nodes).matches(query);
      met(!matches.is_empty(), json!({"met": "element", "matches": matches}))
    }
    UiCondition::Selector(query) => {
      let expression = format!(
        "(() => {{ try {{ return document.querySelector({}) !== null; }} catch (error) {{ return error.message; }} }})()",
        Value::from(query.as_str())
      );
      match evaluate(tab, &expression).await? {
        None => Look::NotYet,
        Some(Value::Bool(found)) => met(found, json!({"met": "selector", "selector": query})),
        Some(Value::String(why)) => return Err(format!("`{query}` is not a CSS selector: {why}")),
        Some(other) => return Err(format!("the page answered {other} for the selector `{query}`")),
      }
    }
    UiCondition::Text(query) => {
      let expression = format!(
        "document.body !== null && document.body.innerText.includes({})",
        Value::from(query.as_str())
      );
      met(
        evaluate(tab, &expression).await? == Some(Value::Bool(true)),
        json!({"met": "text", "text": query}),
      )
    }
    UiCondition::Url(query) => {
      let url = tab.info().await.map_err(|error| error.to_string())?.url;
      met(url.contains(query.as_str()), json!({"met": "url", "url": url}))
    }
  })
}

/// The value of `expression` in the page, a promise awaited; `None` while the tab is between two documents, which
/// refuses it.
async fn evaluate(tab: &Tab, expression: &str) -> Result<Option<Value>, String> {
  let answer = match tab
    .call(
      "Runtime.evaluate",
      json!({ "expression": expression, "returnByValue": true, "awaitPromise": true }),
    )
    .await
  {
    Ok(answer) => answer,
    Err(CdpError::Refused { .. }) => return Ok(None),
    Err(error) => return Err(error.to_string()),
  };
  if let Some(details) = answer.get("exceptionDetails") {
    let thrown = details["exception"]["description"]
      .as_str()
      .or_else(|| details["text"].as_str())
      .unwrap_or("an exception");
    return Err(format!("the page threw: {thrown}"));
  }
  Ok(Some(answer["result"]["value"].clone()))
}
