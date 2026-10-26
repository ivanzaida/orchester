//! The chat's frame at the design's 1440 × 900 (`Orchestrator · redesign` `hujxu`): a one-line 52 header with the
//! title and, at its end, *New* and the History toggle; the conversation and its composer in a 760 column, centred;
//! History 300 wide over the chat's right edge, under the header; a new conversation's start 640 wide.
use orchester_app::routes::OrchestratorChat;
use orchester_core::time::Timestamp;

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
};

fn same(a: f32, b: f32) -> bool {
  (a - b).abs() < 0.5
}

#[test]
fn orchestrator_header_is_one_line_over_a_760_column_and_history_opens_on_the_right() {
  let mut screen = Orchestrator::new(Setup::Ready);
  kept(
    &screen,
    "Plan the GitHub app permissions",
    Timestamp::now(),
    "Auth first.",
  );
  screen.go(OrchestratorChat::Latest);
  screen.until("the conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Auth first.")
  });
  let header = screen.bounds("orchestrator-header");
  assert!(same(header.height, 52.0), "{header:?}");
  for part in ["orchestrator-title", "orchestrator-new", "orchestrator-history"] {
    let bounds = screen.bounds(part);
    assert!(
      bounds.y >= header.y && bounds.y + bounds.height <= header.y + header.height,
      "{part} on the header's line: {bounds:?} in {header:?}"
    );
  }
  assert!(
    !screen.exists("orchestrator-crumbs") && !screen.exists("orchestrator-coordinator"),
    "no breadcrumb and no coordinator line"
  );
  let main = screen.bounds("orchestrator-main");
  let composer = screen.bounds("orchestrator-composer");
  assert!(same(composer.width, 760.0), "{composer:?}");
  let centre = |x: f32, width: f32| x + width / 2.0;
  assert!(
    (centre(composer.x, composer.width) - centre(main.x, main.width)).abs() < 1.0,
    "the composer is centred: {composer:?} in {main:?}"
  );
  screen.click("orchestrator-history");
  screen.until("History", |screen| screen.exists("orchestrator-history-panel"));
  let panel = screen.bounds("orchestrator-history-panel");
  assert!(same(panel.width, 300.0), "{panel:?}");
  assert!(
    same(panel.x + panel.width, main.x + main.width),
    "over the chat's right edge: {panel:?} in {main:?}"
  );
  assert!(same(panel.y, header.y + header.height), "under the header: {panel:?}");
  assert!(
    same(screen.bounds("orchestrator-composer").x, composer.x),
    "History lies over the chat and moves nothing"
  );
}

#[test]
fn orchestrator_new_conversation_starts_640_wide_with_the_composer_focused() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  let start = screen.bounds("orchestrator-start");
  assert!(same(start.width, 640.0), "{start:?}");
  assert_eq!(screen.text("orchestrator-start-title"), "What should we plan?");
  screen.until("the focus in the composer", |screen| {
    screen
      .window
      .tree
      .focused_element()
      .is_some_and(|focused| focused.id() == Some("orchestrator-input"))
  });
}
