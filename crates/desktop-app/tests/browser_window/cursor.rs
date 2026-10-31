//! The mouse cursor over the page (`docs/design/browser.md`, "What frames do not show"): what the window asks the
//! runner as the pointer moves, how often, and the cursor it sets from the answer. The requests are kept and answered
//! by the test, so neither a runner nor a browser runs.

use std::{
  thread,
  time::{Duration, Instant},
};

use lurq::{core::ElementRect, node::CursorIcon};
use orchester_ipc::{
  Rejection,
  browser::{BrowserCommand, BrowserMode, BrowserReply, BrowserTabId, PageCursor},
};

use super::{
  input::{at, frame, on_agents_tab},
  support::{Browser, state},
};

/// The window with its page under a pointer, whose requests wait for the test's answers.
fn answered_by_the_test() -> Option<(Browser, ElementRect)> {
  let (browser, page, _) = on_agents_tab()?;
  browser.state.hold_answers_for_test();
  Some((browser, page))
}

fn asked(tab: &str, x: i32, y: i32) -> BrowserCommand {
  BrowserCommand::CursorAt {
    tab: BrowserTabId::new(tab),
    x,
    y,
  }
}

/// The questions of the cursor's shape among the requests since the last look.
fn questions(browser: &Browser) -> Vec<BrowserCommand> {
  browser
    .state
    .recorded_commands_for_test()
    .into_iter()
    .map(|(_, command)| command)
    .filter(|command| matches!(command, BrowserCommand::CursorAt { .. }))
    .collect()
}

fn move_to(browser: &mut Browser, page: ElementRect, x: f32, y: f32) {
  let (x, y) = at(page, x, y);
  browser.tree.mouse_move(x, y);
}

/// The pointer moves to `(x, y)`, which the window asks about, and `answer` comes.
fn answers(browser: &mut Browser, page: ElementRect, (x, y): (f32, f32), answer: &Result<BrowserReply, Rejection>) {
  move_to(browser, page, x, y);
  let question = asked("T1", x as i32, y as i32);
  browser.until("the window to ask", |browser| questions(browser).contains(&question));
  browser.state.answer_held_for_test(answer);
}

fn says(cursor: PageCursor) -> Result<BrowserReply, Rejection> {
  Ok(BrowserReply::Cursor { cursor })
}

fn until_cursor(browser: &mut Browser, cursor: CursorIcon) {
  browser.until("the cursor to change", |browser| browser.tree.cursor() == cursor);
}

#[test]
fn browser_cursor_over_the_page_takes_the_shape_the_runner_answers() {
  let Some((mut browser, page)) = answered_by_the_test() else {
    return;
  };
  browser.pass();
  assert_eq!(browser.tree.cursor(), CursorIcon::Default, "an arrow before any answer");
  answers(&mut browser, page, (100.0, 50.0), &says(PageCursor::Hand));
  until_cursor(&mut browser, CursorIcon::Pointer);
  answers(&mut browser, page, (120.0, 50.0), &says(PageCursor::Text));
  until_cursor(&mut browser, CursorIcon::Text);
  answers(&mut browser, page, (140.0, 50.0), &says(PageCursor::Arrow));
  until_cursor(&mut browser, CursorIcon::Default);
}

#[test]
fn browser_cursor_is_an_arrow_when_the_runner_refuses_or_answers_otherwise() {
  for failure in [Err(Rejection::NoSuchTab), Ok(BrowserReply::Done)] {
    let Some((mut browser, page)) = answered_by_the_test() else {
      return;
    };
    answers(&mut browser, page, (100.0, 50.0), &says(PageCursor::Hand));
    until_cursor(&mut browser, CursorIcon::Pointer);
    answers(&mut browser, page, (120.0, 50.0), &failure);
    until_cursor(&mut browser, CursorIcon::Default);
  }
}

/// One question at a time, a few a second, the last place the pointer moved to always among them, and none once it
/// stands still.
#[test]
fn browser_cursor_is_asked_for_a_few_times_a_second_and_only_while_the_pointer_moves() {
  let Some((mut browser, page)) = answered_by_the_test() else {
    return;
  };
  assert_eq!(questions(&browser), [], "the pointer has not moved");
  move_to(&mut browser, page, 10.0, 50.0);
  browser.until("the first question", |browser| {
    questions(browser) == [asked("T1", 10, 50)]
  });

  for step in 1..=200 {
    move_to(&mut browser, page, 10.0 + step as f32, 50.0);
  }
  thread::sleep(Duration::from_millis(400));
  assert_eq!(questions(&browser), [], "the first is not answered yet");

  browser.state.answer_held_for_test(&says(PageCursor::Arrow));
  let answered = Instant::now();
  let mut after = Vec::new();
  while after.is_empty() {
    assert!(
      answered.elapsed() < Duration::from_secs(10),
      "the last place is asked about"
    );
    after = questions(&browser);
    thread::sleep(Duration::from_millis(5));
  }
  assert!(
    answered.elapsed() >= Duration::from_millis(200),
    "at most a few questions a second, not after {:?}",
    answered.elapsed()
  );
  assert_eq!(after, [asked("T1", 210, 50)], "only where the pointer stopped");

  browser.state.answer_held_for_test(&says(PageCursor::Arrow));
  thread::sleep(Duration::from_millis(600));
  assert_eq!(questions(&browser), [], "the pointer stands still");
}

/// The answer about a tab is not the next tab's, which starts from the arrow.
#[test]
fn browser_cursor_of_one_tab_does_not_follow_to_another() {
  let Some((mut browser, page)) = answered_by_the_test() else {
    return;
  };
  answers(&mut browser, page, (100.0, 50.0), &says(PageCursor::Hand));
  until_cursor(&mut browser, CursorIcon::Pointer);
  browser.feed(state("T2", BrowserMode::Headless).0);
  frame(&browser, "T2", 2, (page.width as u32, page.height as u32));
  until_cursor(&mut browser, CursorIcon::Default);
}
