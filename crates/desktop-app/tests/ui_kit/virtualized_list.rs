//! lurq 0.49.1's `VirtualizedList` as the chats use it (ORC-71), as small as it repeats: a list of 300 rows 100 high
//! in a 600-high view, asked to open at its end (`scroll_to_bottom_pending`), renders its first 64 rows, from the
//! top, on its first pass, before it has measured one (`BOOTSTRAP_ROWS`), and only then the rows at its end. A list
//! whose rows hold long Markdown replies pays for 64 of them at once when a chat opens, and again when it switches to
//! another conversation (no key survives, which clears its heights and starts over). This is lurq's, reported with
//! the numbers in `docs/plan.md` (ORC-71); the chats' own tests bound what an opening builds by it.

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
const BOOTSTRAP_ROWS: usize = 64;

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
fn virtualized_list_opening_at_its_end_renders_its_first_rows_before_any_is_measured() {
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
  let first: BTreeSet<usize> = (0..BOOTSTRAP_ROWS).collect();
  assert!(
    rendered.is_superset(&first),
    "the first {BOOTSTRAP_ROWS} rows render, from the top: {rendered:?}"
  );
  assert!(rendered.contains(&(ROWS - 1)), "then those at its end: {rendered:?}");
  assert!(
    rendered.len() < BOOTSTRAP_ROWS + 16,
    "and nothing in between: {rendered:?}"
  );
}
