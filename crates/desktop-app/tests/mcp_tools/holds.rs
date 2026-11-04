//! `hold_reads` and `hold_hand_offs` hold the project feeds' reads and the review page's hand-offs, and
//! `hold_integration` needs a project in the active tab.

use orchester_app::{
  mcp::{
    WindowLink,
    tools::{HeldLease, ToolError, hold_hand_offs, hold_integration, hold_reads},
  },
  project::hold::ReadHold,
};
use serde_json::json;

use crate::support::Window;

#[test]
fn hold_reads_holds_and_releases_the_feeds_reads() {
  let hold = ReadHold::default();
  assert_eq!(
    hold_reads(&hold, &json!({ "held": true })).expect("a flag"),
    json!({ "held": true })
  );
  assert!(hold.is_held());
  assert_eq!(
    hold_reads(&hold, &json!({ "held": false })).expect("a flag"),
    json!({ "held": false })
  );
  assert!(!hold.is_held());
  let error = hold_reads(&hold, &json!({ "held": "yes" })).expect_err("not a flag");
  assert_eq!(error.to_json()["code"], "not_a_bool");
  let error = hold_reads(&hold, &json!({})).expect_err("no flag");
  assert_eq!(error.to_json()["code"], "missing_param");
}

#[test]
fn hold_hand_offs_holds_and_releases_the_review_pages_hand_offs() {
  let hold = ReadHold::default();
  assert_eq!(
    hold_hand_offs(&hold, &json!({ "held": true })).expect("a flag"),
    json!({ "held": true })
  );
  assert!(hold.is_held());
  assert_eq!(
    hold_hand_offs(&hold, &json!({ "held": false })).expect("a flag"),
    json!({ "held": false })
  );
  assert!(!hold.is_held());
  let error = hold_hand_offs(&hold, &json!({ "held": 1 })).expect_err("not a flag");
  assert_eq!(error.to_json()["code"], "not_a_bool");
}

#[test]
fn hold_integration_needs_a_project_in_the_active_tab() {
  let lease = HeldLease::default();
  let error = hold_integration(&WindowLink::default(), &lease, &json!({ "held": true })).expect_err("no window yet");
  assert_eq!(error, ToolError::NotReady);
  let window = Window::open();
  let error = hold_integration(&window.link, &lease, &json!({ "held": true })).expect_err("All projects");
  assert_eq!(error.to_json()["code"], "invalid_param");
  assert_eq!(
    hold_integration(&window.link, &lease, &json!({ "held": false })).expect("nothing to let go"),
    json!({ "held": false, "project": null })
  );
  let error = hold_integration(&window.link, &lease, &json!({})).expect_err("no flag");
  assert_eq!(error.to_json()["code"], "missing_param");
}
