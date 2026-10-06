//! The fields of New task and Edit task scroll beside a gutter of their own (design doc, "Scroll
//! gutter"): the thumb never covers a field, and the fields keep their drawn width and right edge,
//! 40 before the plan's divider.

use lurq::{
  app::{events::ScrollPhase, theme::PaletteColor},
  core::ElementRect,
  layout::quad::{Quad, QuadContent},
};

use crate::{
  edit_task::{draft_page, edit},
  harness::Desktop,
  new_task::{close, ends, new_task},
};

/// The scrolling column of fields.
const COLUMN: &str = "new-task-brief";
/// The fields as wide as the column.
const FULL_WIDTH: [&str; 3] = ["new-task-repository", "task-title", "task-brief-area"];
/// The other fields and controls of the column.
const OTHERS: [&str; 5] = [
  "criterion-0",
  "task-workflow-field",
  "task-priority-field",
  "task-blocked-field",
  "attach-context",
];
/// The smallest window the app opens (`main`), and the design's.
const WINDOWS: [(u32, u32); 2] = [(960, 700), (1440, 960)];
/// Between the fields and the plan's divider (`Indent`, `hkWbp`).
const COLUMN_GAP: f32 = 40.0;
/// The scroll gutter's thumb (`nf9DG`): 4 wide, 4 from the gutter's right edge.
const THUMB_WIDTH: f32 = 4.0;
const THUMB_INSET: f32 = 4.0;

/// The scroll thumb painted in `viewport`: a 4 wide `$border` rectangle inside it.
fn thumb(desktop: &mut Desktop, viewport: &ElementRect) -> Option<Quad> {
  let border = desktop.color(PaletteColor::Border);
  let inside = |quad: &Quad| {
    quad.x >= viewport.x
      && quad.x + quad.width <= viewport.x + viewport.width + 0.5
      && quad.y >= viewport.y - 0.5
      && quad.y + quad.height <= viewport.y + viewport.height + 0.5
  };
  desktop
    .tree
    .painted_quads()
    .into_iter()
    .filter(|quad| matches!(quad.content, QuadContent::Rect { color, .. } if color == border))
    .filter(|quad| (quad.width - THUMB_WIDTH).abs() < 0.5 && inside(quad))
    .last()
}

/// A wheel move down over the middle of the column.
fn scroll_down(desktop: &mut Desktop) {
  let column = desktop.bounds(COLUMN);
  let (x, y) = (column.x + column.width / 2.0, column.y + column.height / 2.0);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
  desktop.tree.scroll(x, y, 0.0, -120.0, ScrollPhase::Scroll);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
  desktop.pass();
}

/// In each window the column scrolls, its thumb sits in the gutter right of every field, and the
/// full-width fields end the drawn gap before the plan.
fn assert_thumb_beside_the_fields(desktop: &mut Desktop) {
  for (width, height) in WINDOWS {
    desktop.tree.resize(width, height);
    desktop.pass();
    let before = desktop.bounds("task-title").y;
    scroll_down(desktop);
    assert!(
      desktop.bounds("task-title").y < before,
      "the column scrolls in a {width} × {height} window"
    );
    let column = desktop.bounds(COLUMN);
    let thumb = thumb(desktop, &column).unwrap_or_else(|| panic!("the column shows its thumb at {width} × {height}"));
    assert!(
      close(column.x + column.width - (thumb.x + thumb.width), THUMB_INSET),
      "the thumb at {} is {THUMB_INSET} from the column's edge: {column:?}",
      thumb.x
    );
    let plan = desktop.bounds("execution-plan").x;
    for field in FULL_WIDTH.into_iter().chain(OTHERS) {
      let (right, _) = ends(desktop, field);
      assert!(
        right <= thumb.x,
        "the thumb at {} covers {field}, which ends at {right} ({width} × {height})",
        thumb.x
      );
    }
    for field in FULL_WIDTH {
      let (right, _) = ends(desktop, field);
      assert!(
        close(plan - right, COLUMN_GAP),
        "{field} ends {} before the plan ({width} × {height})",
        plan - right
      );
    }
  }
}

#[test]
fn new_task_scrollbar_keeps_to_its_gutter_beside_the_fields() {
  let mut desktop = new_task();
  assert_thumb_beside_the_fields(&mut desktop);
}

#[test]
fn edit_task_scrollbar_keeps_to_its_gutter_beside_the_fields() {
  let mut desktop = draft_page();
  edit(&mut desktop);
  assert_thumb_beside_the_fields(&mut desktop);
}
