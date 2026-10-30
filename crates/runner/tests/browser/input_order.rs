//! The order of the person's inputs (`docs/design/browser.md`, "The person's input"): an input takes its place when it
//! arrives, whenever its task gets to run, and waits only for the input before it on its own tab.

use std::{pin::pin, time::Duration};

use orchester_core::id::RunId;
use orchester_ipc::{
  Rejection,
  browser::{BrowserCommand, BrowserReply, BrowserTabId, Modifiers, MouseAction, MouseButton, PersonInput, Viewport},
};
use orchester_runner::browser::{Arrived, ProjectBrowser, cdp::TargetId};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::site;
use crate::{browser_installed, support::PATIENCE};

/// How long an input is watched to see that it keeps waiting.
const STILL_WAITING: Duration = Duration::from_millis(200);

fn arrived(browser: &ProjectBrowser, tab: &str, input: PersonInput) -> Arrived {
  browser.arrived(BrowserCommand::Input {
    tab: BrowserTabId::new(tab),
    input,
  })
}

fn mouse(action: MouseAction, (x, y): (i32, i32)) -> PersonInput {
  PersonInput::Mouse {
    action,
    x,
    y,
    button: MouseButton::Left,
    clicks: 1,
    modifiers: Modifiers::default(),
  }
}

fn text(text: &str) -> PersonInput {
  PersonInput::Text { text: text.to_owned() }
}

async fn evaluate(browser: &ProjectBrowser, run: RunId, tab: &TargetId, expression: &str) -> Value {
  let answer = browser
    .call(
      run,
      tab,
      "Runtime.evaluate",
      json!({ "expression": expression, "returnByValue": true }),
    )
    .await
    .expect("the page is evaluated");
  answer["result"]["value"].clone()
}

/// The tasks that carry the requests out start in no certain order, so here the later input always runs first: a press
/// asks the page what lies under it before it goes out, and a release that did not wait would overtake it.
#[tokio::test(flavor = "multi_thread")]
async fn browser_person_inputs_reach_the_page_in_the_order_they_arrived() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = site::serve().await;
  let (desktop, _events) = mpsc::unbounded_channel();
  let run = RunId::generate();
  let target = browser
    .open_tab(run, &format!("{site}window-form"))
    .await
    .expect("opened");
  let tab = target.as_str();
  let show = BrowserCommand::Show {
    tab: BrowserTabId::new(tab),
    viewport: Viewport::new(800, 600, 100).expect("in bounds"),
  };
  browser.serve(browser.arrived(show), &desktop).await.expect("shown");
  let watch = "window.seen = []; for (const name of ['mousedown', 'mouseup', 'click']) \
               addEventListener(name, (event) => seen.push(event.type));";
  evaluate(&browser, run, &target, watch).await;

  let press = arrived(&browser, tab, mouse(MouseAction::Pressed, (150, 120)));
  let release = arrived(&browser, tab, mouse(MouseAction::Released, (150, 120)));
  let (released, pressed) = tokio::join!(browser.serve(release, &desktop), browser.serve(press, &desktop));
  released.expect("the release is carried out");
  pressed.expect("the press is carried out");
  assert_eq!(
    evaluate(&browser, run, &target, "seen.join()").await,
    "mousedown,mouseup,click"
  );

  for action in [MouseAction::Pressed, MouseAction::Released] {
    let on_field = arrived(&browser, tab, mouse(action, (150, 20)));
    browser
      .serve(on_field, &desktop)
      .await
      .expect("the field has the focus");
  }
  let (first, second, third) = (
    arrived(&browser, tab, text("a")),
    arrived(&browser, tab, text("b")),
    arrived(&browser, tab, text("c")),
  );
  let (third, second, first) = tokio::join!(
    browser.serve(third, &desktop),
    browser.serve(second, &desktop),
    browser.serve(first, &desktop),
  );
  for typed in [first, second, third] {
    typed.expect("the text is carried out");
  }
  assert_eq!(
    evaluate(&browser, run, &target, "document.getElementById('field').value").await,
    "abc"
  );
  browser.stop().await;
}

/// No browser runs here, so each input is refused for its tab as soon as its turn comes.
#[tokio::test]
async fn browser_person_input_waits_only_for_the_input_before_it_on_its_tab() {
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let (desktop, _events) = mpsc::unbounded_channel();
  let refused = |served: Result<BrowserReply, Rejection>| assert_eq!(served, Err(Rejection::NoSuchTab));

  let first = arrived(&browser, "A", text("a"));
  let behind = arrived(&browser, "A", text("b"));
  let other_tab = arrived(&browser, "B", text("c"));
  let served = tokio::time::timeout(PATIENCE, browser.serve(other_tab, &desktop)).await;
  refused(served.expect("another tab's input does not wait"));

  let mut waiting = pin!(browser.serve(behind, &desktop));
  assert!(
    tokio::time::timeout(STILL_WAITING, &mut waiting).await.is_err(),
    "the input behind waits for the one before it"
  );
  refused(browser.serve(first, &desktop).await);
  let served = tokio::time::timeout(PATIENCE, waiting).await;
  refused(served.expect("the input behind goes once the one before it has gone"));

  let dropped = arrived(&browser, "C", text("d"));
  let next = arrived(&browser, "C", text("e"));
  drop(dropped);
  let served = tokio::time::timeout(PATIENCE, browser.serve(next, &desktop)).await;
  refused(served.expect("an input never carried out holds nothing up"));
}
