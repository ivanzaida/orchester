//! lurq's `VirtualizedList` as the chats use it (ORC-71): a list of 300 rows 100 high in a 600-high view, asked to
//! open at its end (`scroll_to_bottom_pending`), renders on its first pass, before it has measured one row, only its
//! last bootstrap rows (16 since lurq 0.50.0) and none from its top. lurq 0.49.1 rendered its first 64 rows from the
//! top before the rows at its end, so a chat whose rows hold long Markdown replies paid for 64 of them at once.

use std::{collections::BTreeSet, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Column, VirtualizedList},
  layout::layout_kind::ScrollState,
  node::dimension::Dimension,
};
use parking_lot::Mutex;

use super::harness::Kit;

/// Rows in the list.
const ROWS: usize = 300;
/// What lurq renders before it has measured a row.
const BOOTSTRAP_ROWS: usize = 16;

/// A row, and where the rows note that they rendered.
#[derive(Clone)]
struct Line(usize, Arc<Mutex<BTreeSet<usize>>>);

impl PartialEq for Line {
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}

impl DevtoolsInspectable for Line {}

struct LineView;

impl Component for LineView {
  type Props = Line;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<lurq::node::Element> {
    let Line(index, rendered) = ctx.props::<Line>().clone();
    rendered.lock().insert(index);
    Column::new().width(Dimension::Pct(100.0)).height(Dimension::Px(100.0))
  }
}

#[test]
fn virtualized_list_opening_at_its_end_renders_only_its_last_rows_before_any_is_measured() {
  let rendered = Arc::new(Mutex::new(BTreeSet::new()));
  let scroll = ScrollState::new();
  scroll.scroll_to_bottom_pending();
  let lines: Arc<Vec<Line>> = Arc::new((0..ROWS).map(|index| Line(index, rendered.clone())).collect());
  let mut kit = Kit::show(move |ctx| {
    Column::new()
      .width(Dimension::Px(800.0))
      .height(Dimension::Px(600.0))
      .child(
        VirtualizedList::new_shared(ctx, lines.clone())
          .with_scroll_state(scroll.clone())
          .list_key("vlist")
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .mount_keyed::<LineView, _, _, _>(|line| line.0, Clone::clone),
      )
      .into()
  });
  kit.pass();
  let rendered = rendered.lock().clone();
  let last: BTreeSet<usize> = (ROWS - BOOTSTRAP_ROWS..ROWS).collect();
  assert_eq!(
    rendered, last,
    "only the last {BOOTSTRAP_ROWS} rows render, none from the top"
  );
}
