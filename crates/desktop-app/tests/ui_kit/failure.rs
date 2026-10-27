//! The failure states (`Failure states · redesign` `SsC16`): a section that can't be read is one row in place of its
//! rows under its heading while the rest of the page works; a page that can't be read is a card whose Try again reads
//! in place and whose success replaces it; a wait on the database is information.

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use lurq::{
  app::{component::DevtoolsInspectable, theme::SpacingSize},
  components::{Column, Text},
  core::Signal,
  node::dimension::Dimension,
};
use orchester_app::ui::{
  button::{ActionButton, ButtonKind},
  failure::Failure,
};
use orchester_workspace::messages::Message;

use crate::harness::Kit;

/// How the read a test drives stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Read {
  Failed,
  Retrying,
  Done,
}

impl DevtoolsInspectable for Read {}

#[test]
fn a_section_that_cannot_be_read_keeps_its_heading_and_the_rest_of_the_page_works() {
  let (retried, pressed) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
  let (retry, press) = (retried.clone(), pressed.clone());
  let mut kit = Kit::show(move |ctx| {
    let (retry, press) = (retry.clone(), press.clone());
    Column::new()
      .width(Dimension::Px(720.0))
      .spacing(SpacingSize::Md)
      .child(Text::new("Limits").id("limits-head"))
      .child(
        Failure::new("limits", "Limits couldn't be read", Message::new("errors.storage"))
          .retry(false, move || {
            retry.fetch_add(1, Ordering::SeqCst);
          })
          .section(ctx),
      )
      .child(
        ActionButton::new("other-setting", ButtonKind::Secondary, "Other setting").on_click(move || {
          press.fetch_add(1, Ordering::SeqCst);
        }),
      )
      .into()
  });
  assert!(kit.exists("limits-head"), "the group keeps its heading");
  let row = kit.texts("limits");
  assert!(row.contains(&"Limits couldn't be read".to_owned()), "{row:?}");
  assert!(
    row.iter().any(|text| text.contains("The project's database couldn't be read")),
    "why: {row:?}"
  );
  kit.click("limits-retry");
  assert_eq!(retried.load(Ordering::SeqCst), 1, "Try again reads the section again");
  kit.click("other-setting");
  assert_eq!(pressed.load(Ordering::SeqCst), 1, "the rest of the page works");
}

#[test]
fn a_page_card_reads_again_in_place_and_its_success_replaces_it() {
  let read = Signal::new(Read::Failed);
  let shown = read.clone();
  let mut kit = Kit::show(move |ctx| {
    let again = shown.clone();
    match shown.get() {
      Read::Done => Text::new("The profiles").id("content").into(),
      state => Failure::new("page", "Agent profiles couldn't be read", Message::new("errors.storage"))
        .retry(state == Read::Retrying, move || again.set(Read::Retrying))
        .still_works("Runs keep going.")
        .page(ctx),
    }
  });
  let texts = kit.texts("page");
  assert!(texts.contains(&"Agent profiles couldn't be read".to_owned()), "{texts:?}");
  assert!(texts.contains(&"Runs keep going.".to_owned()), "{texts:?}");
  kit.click("page-retry");
  assert!(
    kit.texts("page-retry").contains(&"Trying again…".to_owned()),
    "the card stays and says it reads again"
  );
  read.set(Read::Failed);
  kit.pass();
  kit.click("page-details-toggle");
  assert!(kit.texts("page-tries").iter().any(|t| t.starts_with("Tried 2 times")));
  read.set(Read::Done);
  kit.pass();
  assert!(kit.exists("content") && !kit.exists("page"), "success replaces the card");
}

#[test]
fn an_item_that_cannot_be_read_keeps_its_place_and_its_name() {
  let mut kit = Kit::show(|ctx| {
    Column::new()
      .width(Dimension::Px(720.0))
      .child(Text::new("Builder").id("builder"))
      .child(Failure::new("reviewer", "Reviewer couldn't be read", Message::new("errors.storage")).row(ctx, Some("Reviewer")))
      .child(Text::new("Explorer").id("explorer"))
      .into()
  });
  let row = kit.texts("reviewer");
  assert!(row.contains(&"Reviewer".to_owned()), "its name: {row:?}");
  assert!(
    row.iter().any(|t| t.starts_with("Couldn't be read · The data could not be read")),
    "{row:?}"
  );
  let (above, failed, below) = (kit.bounds("builder"), kit.bounds("reviewer"), kit.bounds("explorer"));
  assert!(above.y < failed.y && failed.y < below.y, "the list keeps its order");
}

#[test]
fn a_busy_database_is_information_not_an_error() {
  let mut kit = Kit::show(|ctx| {
    Failure::new("busy", "Agent profiles couldn't be read", Message::new("errors.databaseLocked"))
      .retry(false, || {})
      .page(ctx)
  });
  let texts = kit.texts("busy");
  assert!(texts.contains(&"The project's database is busy".to_owned()), "{texts:?}");
  assert!(
    texts.iter().any(|t| t.contains("tries again on its own")),
    "{texts:?}"
  );
  assert!(!kit.exists("busy-retry"), "it tries again on its own");
}
