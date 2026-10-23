//! lurq's `VirtualizedList` at its end as a chat keeps it there (`ui::chat::follow`): 50 rows 100 high in a
//! 600-high view, scrolled to its end; then a new last row comes whose measured height (1,500) is far over the list's
//! estimate (100), or the last row grows to that height, and the window draws only the frames it asks for. lurq
//! lands a queued `scroll_to_bottom_pending` on the measured end and sticks to a growing last row; the chat's
//! `Follow` asks for the end on each arrival.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Column, VirtualizedList},
  core::Signal,
  layout::layout_kind::ScrollState,
  node::dimension::Dimension,
};
use parking_lot::Mutex;

use super::harness::Kit;

/// A row: its key and its height.
#[derive(Clone, Copy, PartialEq)]
struct Line(usize, f32);

impl DevtoolsInspectable for Line {}

struct LineView;

impl Component for LineView {
  type Props = Line;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<lurq::node::Element> {
    let Line(key, height) = *ctx.props::<Line>();
    Column::new()
      .id(format!("line-{key}"))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(height))
  }
}

/// The rows the list shows and the signal that redraws it, shared with the test.
#[derive(Clone)]
struct Feed {
  rows: Arc<Mutex<Vec<Line>>>,
  scroll: ScrollState,
  redraw: Arc<Mutex<Option<Signal<u64>>>>,
}

impl PartialEq for Feed {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.rows, &other.rows)
  }
}

impl DevtoolsInspectable for Feed {}

struct FeedView {
  revision: Signal<u64>,
}

impl Component for FeedView {
  type Props = Feed;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      revision: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<lurq::node::Element> {
    let feed = ctx.props::<Feed>().clone();
    self.revision.get();
    *feed.redraw.lock() = Some(self.revision.clone());
    let rows = Arc::new(feed.rows.lock().clone());
    Column::new()
      .width(Dimension::Px(800.0))
      .height(Dimension::Px(600.0))
      .child(
        VirtualizedList::new_shared(ctx, rows)
          .with_scroll_state(feed.scroll)
          .list_key("follow-vlist")
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .mount_keyed::<LineView, _, _, _>(|line| line.0, Clone::clone),
      )
  }
}

/// A list of 50 rows 100 high, opened at its end and settled there.
fn at_end() -> (Kit, Feed) {
  let feed = Feed {
    rows: Arc::new(Mutex::new((0..50).map(|key| Line(key, 100.0)).collect())),
    scroll: ScrollState::new(),
    redraw: Arc::default(),
  };
  feed.scroll.scroll_to_bottom_pending();
  let shown = feed.clone();
  let mut kit = Kit::show(move |ctx| ctx.mount_keyed::<FeedView>("feed", shown.clone()));
  frames(&mut kit);
  assert!(
    gap_to_end(&feed) < 1.0,
    "the list opened at its end: {} short of it",
    gap_to_end(&feed)
  );
  (kit, feed)
}

/// Draws the frames the window asks for, as its event loop does.
fn frames(kit: &mut Kit) {
  for _ in 0..30 {
    kit.tree.tick_futures();
    if !kit.tree.needs_redraw() {
      return;
    }
    kit.pass();
  }
}

/// How far the list's scroll is short of its end.
fn gap_to_end(feed: &Feed) -> f32 {
  let end = (feed.scroll.content_height() - feed.scroll.viewport_height()).max(0.0);
  end - feed.scroll.scroll_y()
}

/// Changes the rows with `change` and redraws the list.
fn change(feed: &Feed, change: impl FnOnce(&mut Vec<Line>)) {
  change(&mut feed.rows.lock());
  let redraw = feed.redraw.lock().clone().expect("the list rendered");
  redraw.update(|revision| *revision += 1);
}

#[test]
fn virtualized_list_at_its_end_asked_to_its_end_shows_the_end_of_a_new_row_taller_than_its_estimate() {
  let (mut kit, feed) = at_end();
  change(&feed, |rows| rows.push(Line(50, 1500.0)));
  feed.scroll.scroll_to_bottom_pending();
  frames(&mut kit);
  let gap = gap_to_end(&feed);
  assert!(gap < 1.0, "the list is at the new row's end: {gap} short of it");
}

#[test]
fn virtualized_list_at_its_end_shows_the_end_of_a_last_row_that_grows() {
  let (mut kit, feed) = at_end();
  change(&feed, |rows| rows[49].1 = 1500.0);
  frames(&mut kit);
  let gap = gap_to_end(&feed);
  assert!(gap < 1.0, "the list stays at the grown row's end: {gap} short of it");
}

/// lurq keeps the rows in view where they were when a new one comes and nothing asks for the end: following is the
/// app's (`ui::chat::follow`), which asks on every arrival.
#[test]
fn virtualized_list_at_its_end_not_asked_keeps_a_new_row_under_the_view() {
  let (mut kit, feed) = at_end();
  let before = feed.scroll.scroll_y();
  change(&feed, |rows| rows.push(Line(50, 1500.0)));
  frames(&mut kit);
  let after = feed.scroll.scroll_y();
  assert!(
    (after - before).abs() < 1.0,
    "the rows in view stay: {before} then {after}"
  );
}

/// The rows a chat shows through `ui::chat::follow::Follow`, and the signal that redraws it.
#[derive(Clone)]
struct Chat {
  rows: Arc<Mutex<Vec<Line>>>,
  redraw: Arc<Mutex<Option<Signal<u64>>>>,
}

impl PartialEq for Chat {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.rows, &other.rows)
  }
}

impl DevtoolsInspectable for Chat {}

struct ChatView {
  revision: Signal<u64>,
  follow: orchester_app::ui::chat::follow::Follow,
}

impl Component for ChatView {
  type Props = Chat;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      revision: ctx.signal(0),
      follow: orchester_app::ui::chat::follow::Follow::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<lurq::node::Element> {
    let chat = ctx.props::<Chat>().clone();
    self.revision.get();
    *chat.redraw.lock() = Some(self.revision.clone());
    let rows = Arc::new(chat.rows.lock().clone());
    let reveal = self.follow.follow(rows.len(), rows.len(), None);
    let list = self
      .follow
      .list(VirtualizedList::new_shared(ctx, rows.clone()), rows.len(), reveal)
      .list_key("chat-vlist")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .mount_keyed::<LineView, _, _, _>(|line| line.0, Clone::clone);
    Column::new()
      .id("chat-view")
      .width(Dimension::Px(800.0))
      .height(Dimension::Px(600.0))
      .child(list)
      .child(self.follow.end(ctx, "chat-follow-end"))
  }
}

/// The chat's own following (`Follow`) at its end: a new row far taller than the list's estimate ends in view.
#[test]
fn virtualized_list_followed_chat_shows_the_end_of_a_new_row_taller_than_its_estimate() {
  let chat = Chat {
    rows: Arc::new(Mutex::new((0..50).map(|key| Line(key, 100.0)).collect())),
    redraw: Arc::default(),
  };
  let shown = chat.clone();
  let mut kit = Kit::show(move |ctx| ctx.mount_keyed::<ChatView>("chat", shown.clone()));
  frames(&mut kit);
  let foot = |kit: &mut Kit, key: usize| {
    let view = kit.bounds("chat-view");
    let line = kit.bounds(&format!("line-{key}"));
    (view.y + view.height) - (line.y + line.height)
  };
  assert!(foot(&mut kit, 49).abs() < 1.0, "the chat opened at its end");
  chat.rows.lock().push(Line(50, 1500.0));
  let redraw = chat.redraw.lock().clone().expect("the chat rendered");
  redraw.update(|revision| *revision += 1);
  frames(&mut kit);
  let gap = foot(&mut kit, 50);
  assert!(gap.abs() < 1.0, "the new row ends at the chat's foot: {gap} above it");
}
