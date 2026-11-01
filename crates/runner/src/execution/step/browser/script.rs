//! `browser_wait` and `browser_eval`: waiting until a text or an element shows in a page, within the call's time
//! limit, and a JavaScript expression's value, which a person approved before the call ran.

use std::time::Duration;

use orchester_core::orchester_tools::browser::{PAGE_DATA_NOTICE, WaitFor};
use serde_json::{Value, json};
use tokio::time::Instant;

use super::{ToolError, act};
use crate::browser::{
  BrowserError, Tab,
  cdp::{COMMAND_LIMIT, CdpError},
};

/// How often a wait looks at the page again.
const WAIT_POLL: Duration = Duration::from_millis(200);

/// Waits until `until` shows in `tab`'s page, at most `seconds`.
pub(super) async fn wait(tab: &Tab, until: &WaitFor, seconds: u32) -> Result<String, ToolError> {
  let deadline = Instant::now() + Duration::from_secs(u64::from(seconds));
  let started = Instant::now();
  loop {
    if shows(tab, until).await? {
      return Ok(format!(
        "{} showed after {:.1} s.",
        described(until),
        started.elapsed().as_secs_f32()
      ));
    }
    if Instant::now() + WAIT_POLL > deadline {
      return Err(ToolError::Waited {
        what: described(until),
        seconds,
      });
    }
    tokio::time::sleep(WAIT_POLL).await;
  }
}

/// Whether `until` shows in `tab`'s page now. A ref the tab's last read did not name is an error at once.
async fn shows(tab: &Tab, until: &WaitFor) -> Result<bool, ToolError> {
  match until {
    WaitFor::Text(text) => {
      let expression = format!(
        "document.body !== null && document.body.innerText.includes({})",
        Value::from(text.as_str())
      );
      let answer = tab
        .call(
          "Runtime.evaluate",
          json!({ "expression": expression, "returnByValue": true }),
        )
        .await?;
      Ok(answer["result"]["value"] == Value::Bool(true))
    }
    // An element without a box, or one the page no longer holds, has not shown yet.
    WaitFor::Element(element) => match act::centre(tab, element).await {
      Ok(_) => Ok(true),
      Err(ToolError::Page(_) | ToolError::Browser(BrowserError::Cdp(CdpError::Refused { .. }))) => Ok(false),
      Err(error) => Err(error),
    },
  }
}

fn described(until: &WaitFor) -> String {
  match until {
    WaitFor::Text(text) => format!("The text \"{text}\""),
    WaitFor::Element(element) => format!("The element {}", element.as_str()),
  }
}

/// The value of `expression` in `tab`'s page as JSON, a promise awaited, within the browser's time limit.
pub(super) async fn eval(tab: &Tab, expression: &str) -> Result<String, ToolError> {
  let answer = tab
    .call_within(
      "Runtime.evaluate",
      json!({ "expression": expression, "returnByValue": true, "awaitPromise": true, "userGesture": true }),
      COMMAND_LIMIT,
    )
    .await?;
  if let Some(details) = answer.get("exceptionDetails") {
    let thrown = details["exception"]["description"]
      .as_str()
      .or_else(|| details["text"].as_str())
      .unwrap_or("an exception");
    return Err(ToolError::Page(format!("the expression threw: {thrown}")));
  }
  let result = &answer["result"];
  let value = match result.get("value") {
    Some(value) => value.to_string(),
    None => result["description"]
      .as_str()
      .or_else(|| result["type"].as_str())
      .unwrap_or("undefined")
      .to_owned(),
  };
  Ok(format!("{PAGE_DATA_NOTICE}\n{value}"))
}
