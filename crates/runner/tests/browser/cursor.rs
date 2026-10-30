//! The mouse cursor's shape as the runner tells it, with a real browser (`docs/design/browser.md`, "What frames do not
//! show"): a hand over links and buttons, a text cursor over fields, and an arrow over plain text, over nothing, for
//! what cannot be pressed and for a cursor of a page's own that the window does not draw.

use orchester_core::id::RunId;
use orchester_ipc::{
  Rejection,
  browser::{BrowserCommand, BrowserReply, BrowserTabId, PageCursor},
};
use orchester_runner::browser::ProjectBrowser;

use super::{Desktop, browser_installed, pages, tab_of, viewport};

/// What the cursor looks like at `(x, y)` of `tab`.
async fn cursor(desktop: &Desktop, browser: &ProjectBrowser, tab: &BrowserTabId, (x, y): (i32, i32)) -> PageCursor {
  let asked = BrowserCommand::CursorAt { tab: tab.clone(), x, y };
  match desktop.serve(browser, asked).await {
    BrowserReply::Cursor { cursor } => cursor,
    other => panic!("expected the cursor's shape, got {other:?}"),
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_cursor_is_a_hand_over_links_and_buttons_a_text_cursor_over_fields_and_else_an_arrow() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let desktop = Desktop::new();
  let target = browser
    .open_tab(RunId::generate(), &format!("{site}window-cursor"))
    .await
    .expect("opened");
  let tab = tab_of(&target);
  let show = BrowserCommand::Show {
    tab: tab.clone(),
    viewport: viewport(),
  };
  desktop.serve(&browser, show).await;

  for (place, at, expected) in [
    ("a link", (10, 10), PageCursor::Hand),
    ("a word of a link", (150, 10), PageCursor::Hand),
    ("a button", (10, 60), PageCursor::Hand),
    ("a disabled button", (10, 110), PageCursor::Arrow),
    ("a page's own hand", (10, 160), PageCursor::Hand),
    ("a page's own cursor", (10, 210), PageCursor::Arrow),
    ("a field", (10, 260), PageCursor::Text),
    ("a text area", (10, 310), PageCursor::Text),
    ("an editable element", (10, 360), PageCursor::Text),
    ("plain text", (10, 410), PageCursor::Arrow),
    ("nothing but the page", (700, 500), PageCursor::Arrow),
    ("nothing at all", (5000, 5000), PageCursor::Arrow),
  ] {
    assert_eq!(cursor(&desktop, &browser, &tab, at).await, expected, "over {place}");
  }

  let gone = BrowserTabId::new("no such tab");
  let refused = browser
    .serve(
      browser.arrived(BrowserCommand::CursorAt { tab: gone, x: 1, y: 1 }),
      &desktop.sender,
    )
    .await;
  assert_eq!(
    refused,
    Err(Rejection::NoSuchTab),
    "the window takes this for the arrow"
  );
  browser.stop().await;
}
