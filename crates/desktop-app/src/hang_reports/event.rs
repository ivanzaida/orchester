//! The reports of a hang, built here and sent through the client's scrubber like every other report.

use std::time::Duration;

use sentry::protocol::{Addr, Event, Exception, Frame, Level, Map, Stacktrace};

use super::stack::UiStack;

/// The error type of a hang's report.
pub const NOT_RESPONDING: &str = "ApplicationNotResponding";

/// The tag that names the kind of screen the window showed (`Screen::name`).
pub const SCREEN_TAG: &str = "screen";

/// The tag that says whether the report carries the window's thread's stack: [`CAPTURED`], or why not.
pub const STACK_TAG: &str = "stack";

pub const CAPTURED: &str = "captured";

/// *Application not responding*: the window's thread has given no sign of life for `quiet` while it showed `screen`,
/// with its `stack` as an exception's stack trace, outermost frame first as the protocol orders them.
pub(super) fn not_responding(quiet: Duration, screen: &str, stack: UiStack) -> Event<'static> {
  let seconds = quiet.as_secs();
  let (stacktrace, stack_tag) = match stack {
    UiStack::Captured(addresses) => (Some(stacktrace(&addresses)), CAPTURED),
    UiStack::NotCaptured(reason) => (None, reason),
  };
  let value = match stacktrace {
    Some(_) => format!("the window has not responded for {seconds} s"),
    None => format!("the window has not responded for {seconds} s; its stack was {stack_tag}"),
  };
  Event {
    level: Level::Error,
    exception: vec![Exception {
      ty: NOT_RESPONDING.to_owned(),
      value: Some(value),
      stacktrace,
      ..Exception::default()
    }]
    .into(),
    tags: tags(screen, Some(stack_tag)),
    ..Event::default()
  }
}

/// The window's thread, reported not responding, lives again after `hung` without a sign of life.
pub(super) fn responding(hung: Duration, screen: &str) -> Event<'static> {
  Event {
    level: Level::Info,
    message: Some(format!("the window responds again after {} s", hung.as_secs())),
    tags: tags(screen, None),
    ..Event::default()
  }
}

fn stacktrace(addresses: &[u64]) -> Stacktrace {
  Stacktrace {
    frames: addresses
      .iter()
      .rev()
      .map(|&address| Frame {
        instruction_addr: Some(Addr(address)),
        ..Frame::default()
      })
      .collect(),
    ..Stacktrace::default()
  }
}

fn tags(screen: &str, stack: Option<&str>) -> Map<String, String> {
  let mut tags = Map::new();
  tags.insert(SCREEN_TAG.to_owned(), screen.to_owned());
  if let Some(stack) = stack {
    tags.insert(STACK_TAG.to_owned(), stack.to_owned());
  }
  tags
}
