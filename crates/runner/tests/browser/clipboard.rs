//! The person's copy, cut and paste as the runner serves them, with a real browser (`docs/design/browser.md`, "What
//! frames do not show"): the selection is read from the page's text or from the field with the focus, a cut deletes it
//! where it can be edited, a paste is an input like any other, and a copy waits for the inputs that arrived before it.

use orchester_core::id::RunId;
use orchester_ipc::browser::{
  BrowserCommand, BrowserReply, BrowserTabId, Modifiers, MouseAction, MouseButton, PersonInput,
};
use orchester_runner::browser::{ProjectBrowser, cdp::TargetId};
use tempfile::TempDir;

use super::{Desktop, browser_installed, click, evaluate, pages, person_tab, tab_of, viewport};

/// A run's tab on the clipboard page, shown in the window, where the test can also ask the page what it holds.
struct Page {
  browser: ProjectBrowser,
  desktop: Desktop,
  run: RunId,
  target: TargetId,
  tab: BrowserTabId,
  _profile: TempDir,
}

impl Page {
  async fn open() -> Self {
    let profile = tempfile::tempdir().expect("temporary folder");
    let browser = ProjectBrowser::new(profile.path().to_path_buf());
    let site = pages().await;
    let desktop = Desktop::new();
    let run = RunId::generate();
    let target = browser
      .open_tab(run, &format!("{site}window-clipboard"))
      .await
      .expect("opened");
    let tab = tab_of(&target);
    let page = Self {
      browser,
      desktop,
      run,
      target,
      tab,
      _profile: profile,
    };
    let show = BrowserCommand::Show {
      tab: page.tab.clone(),
      viewport: viewport(),
    };
    page.desktop.serve(&page.browser, show).await;
    page
  }

  async fn evaluate(&self, expression: &str) -> serde_json::Value {
    evaluate(&self.browser, self.run, &self.target, expression).await
  }

  async fn click(&self, at: (i32, i32)) {
    click(&self.desktop, &self.browser, &self.tab, at).await;
  }

  async fn input(&self, input: PersonInput) {
    let input = BrowserCommand::Input {
      tab: self.tab.clone(),
      input,
    };
    self.desktop.serve(&self.browser, input).await;
  }

  async fn copy(&self) -> String {
    selection(
      &self.desktop,
      &self.browser,
      BrowserCommand::CopySelection { tab: self.tab.clone() },
    )
    .await
  }

  async fn cut(&self) -> String {
    selection(
      &self.desktop,
      &self.browser,
      BrowserCommand::CutSelection { tab: self.tab.clone() },
    )
    .await
  }
}

async fn selection(desktop: &Desktop, browser: &ProjectBrowser, command: BrowserCommand) -> String {
  match desktop.serve(browser, command).await {
    BrowserReply::Selection { text } => text,
    other => panic!("expected the selection, got {other:?}"),
  }
}

fn text(text: &str) -> PersonInput {
  PersonInput::Text { text: text.to_owned() }
}

/// Where the page's fields are.
const FIELD: (i32, i32) = (150, 20);
const PASSWORD: (i32, i32) = (150, 70);
const EMAIL: (i32, i32) = (150, 120);
const READ_ONLY: (i32, i32) = (150, 170);
const EDITABLE: (i32, i32) = (150, 220);

const FIELD_VALUE: &str = "document.getElementById('field').value";

#[tokio::test(flavor = "multi_thread")]
async fn browser_person_copies_the_selected_text_of_the_page() {
  if !browser_installed() {
    return;
  }
  let page = Page::open().await;
  assert_eq!(page.copy().await, "", "nothing is selected yet");
  page
    .evaluate("getSelection().selectAllChildren(document.getElementById('words'))")
    .await;
  assert_eq!(page.copy().await, "Quiet words on the page");
  assert_eq!(page.cut().await, "Quiet words on the page", "a cut copies it");
  assert_eq!(
    page.evaluate("document.getElementById('words').textContent").await,
    "Quiet words on the page",
    "and deletes nothing that cannot be edited"
  );
  page.browser.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_person_copies_and_cuts_the_selection_of_a_field() {
  if !browser_installed() {
    return;
  }
  let page = Page::open().await;
  page.click(FIELD).await;
  page.input(text("hello world")).await;
  assert_eq!(page.copy().await, "", "only a caret in the field");

  page
    .evaluate("document.getElementById('field').setSelectionRange(0, 5)")
    .await;
  assert_eq!(page.copy().await, "hello");
  assert_eq!(page.evaluate(FIELD_VALUE).await, "hello world", "a copy leaves it");

  assert_eq!(page.evaluate("document.title = 'unchanged'").await, "unchanged");
  assert_eq!(page.cut().await, "hello");
  assert_eq!(page.evaluate(FIELD_VALUE).await, " world", "a cut deletes it");
  assert_eq!(
    page.evaluate("document.title").await,
    "changed",
    "as an edit of the person's, which the page hears of"
  );
  page.browser.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_person_cuts_as_each_kind_of_field_allows() {
  if !browser_installed() {
    return;
  }
  let page = Page::open().await;
  page.click(PASSWORD).await;
  page.evaluate("document.getElementById('secret').select()").await;
  assert_eq!(page.copy().await, "", "a password field never gives its text");
  assert_eq!(page.cut().await, "");
  assert_eq!(
    page.evaluate("document.getElementById('secret').value").await,
    "hunter2"
  );

  page.click(EMAIL).await;
  page.evaluate("document.getElementById('mail').select()").await;
  assert_eq!(
    page.copy().await,
    "ada@example.com",
    "a field without a selection range"
  );
  assert_eq!(page.cut().await, "ada@example.com");
  assert_eq!(page.evaluate("document.getElementById('mail').value").await, "");

  page.click(READ_ONLY).await;
  page.evaluate("document.getElementById('fixed').select()").await;
  assert_eq!(page.cut().await, "read only");
  assert_eq!(
    page.evaluate("document.getElementById('fixed').value").await,
    "read only"
  );

  page.click(EDITABLE).await;
  page
    .evaluate("getSelection().selectAllChildren(document.getElementById('notes'))")
    .await;
  assert_eq!(page.cut().await, "editable notes");
  assert_eq!(page.evaluate("document.getElementById('notes').textContent").await, "");
  page.browser.stop().await;
}

/// A paste is the clipboard's text as an input: it goes in at the caret, or in place of the selection.
#[tokio::test(flavor = "multi_thread")]
async fn browser_person_pastes_into_a_field() {
  if !browser_installed() {
    return;
  }
  let page = Page::open().await;
  page.click(FIELD).await;
  page.input(text("pasted")).await;
  assert_eq!(page.evaluate(FIELD_VALUE).await, "pasted");
  page
    .evaluate("document.getElementById('field').setSelectionRange(0, 2)")
    .await;
  page.input(text("re")).await;
  assert_eq!(page.evaluate(FIELD_VALUE).await, "rested");
  page.browser.stop().await;
}

/// The person's own tab, which no script of the test reaches, with a selection made by a double click.
#[tokio::test(flavor = "multi_thread")]
async fn browser_person_cuts_a_double_clicked_word_on_their_own_tab() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let desktop = Desktop::new();
  let tab = person_tab(&desktop, &browser, Some(format!("{site}window-clipboard"))).await;
  let show = BrowserCommand::Show {
    tab: tab.clone(),
    viewport: viewport(),
  };
  desktop.serve(&browser, show).await;
  click(&desktop, &browser, &tab, FIELD).await;
  let typed = BrowserCommand::Input {
    tab: tab.clone(),
    input: text("hello"),
  };
  desktop.serve(&browser, typed).await;
  for action in [MouseAction::Pressed, MouseAction::Released] {
    let input = PersonInput::Mouse {
      action,
      x: FIELD.0,
      y: FIELD.1,
      button: MouseButton::Left,
      clicks: 2,
      modifiers: Modifiers::default(),
    };
    let second = BrowserCommand::Input {
      tab: tab.clone(),
      input,
    };
    desktop.serve(&browser, second).await;
  }

  let cut = BrowserCommand::CutSelection { tab: tab.clone() };
  assert_eq!(selection(&desktop, &browser, cut).await, "hello");
  let copy = BrowserCommand::CopySelection { tab };
  assert_eq!(selection(&desktop, &browser, copy).await, "", "it was deleted");
  browser.stop().await;
}

/// The tasks that carry the requests out start in no certain order: here the copy runs first, and still waits for the
/// text typed before it, which replaced the selection.
#[tokio::test(flavor = "multi_thread")]
async fn browser_person_copy_waits_for_the_inputs_before_it() {
  if !browser_installed() {
    return;
  }
  let page = Page::open().await;
  page.click(FIELD).await;
  page.input(text("hello")).await;
  page
    .evaluate("document.getElementById('field').setSelectionRange(0, 5)")
    .await;
  let typed = page.browser.arrived(BrowserCommand::Input {
    tab: page.tab.clone(),
    input: text("bye"),
  });
  let copied = page
    .browser
    .arrived(BrowserCommand::CopySelection { tab: page.tab.clone() });
  let copying = page.browser.serve(copied, &page.desktop.sender);
  let typing = page.browser.serve(typed, &page.desktop.sender);
  let (copied, typed) = tokio::join!(copying, typing);
  typed.expect("typed");
  assert_eq!(copied.expect("copied"), BrowserReply::Selection { text: String::new() });
  assert_eq!(page.evaluate(FIELD_VALUE).await, "bye");
  page.browser.stop().await;
}
