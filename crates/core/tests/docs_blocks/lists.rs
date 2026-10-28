use orchester_core::docs::blocks::{BlockKind, Checkbox, ListItem, ListMarker};

use super::support::{bullet, kinds, normalized, ordered, paragraph, plain, round_trip};

fn task(task: Checkbox, depth: usize, text: &str) -> BlockKind {
  BlockKind::ListItem(ListItem {
    marker: ListMarker::Bullet,
    task: Some(task),
    depth,
    text: plain(text),
  })
}

#[test]
fn docs_blocks_list_items_keep_kind_number_and_nesting() {
  let markdown = "* one\n  + nested\n    - deeper\n* two\n\n3) three\n4) four\n   1. inner\n\n- [ ] open\n- [x] done\n";
  assert_eq!(
    kinds(markdown),
    vec![
      bullet(0, "one"),
      bullet(1, "nested"),
      bullet(2, "deeper"),
      bullet(0, "two"),
      ordered(3, 0, "three"),
      ordered(4, 0, "four"),
      ordered(1, 1, "inner"),
      task(Checkbox::Unchecked, 0, "open"),
      task(Checkbox::Checked, 0, "done"),
    ]
  );
  assert_eq!(
    normalized(markdown),
    "- one\n  - nested\n    - deeper\n- two\n\n3. three\n4. four\n   1. inner\n\n- [ ] open\n- [x] done\n"
  );
}

#[test]
fn docs_blocks_loose_items_are_tight_and_their_later_paragraphs_are_blocks() {
  let markdown = "- first\n\n  more of first\n\n- second\n";
  assert_eq!(
    kinds(markdown),
    vec![bullet(0, "first"), paragraph("more of first"), bullet(0, "second")]
  );
  assert_eq!(normalized(markdown), "- first\n\nmore of first\n\n- second\n");
}

#[test]
fn docs_blocks_an_item_after_another_block_starts_at_level_zero() {
  let markdown = "- a\n\n  para\n\n  - b\n";
  assert_eq!(kinds(markdown), vec![bullet(0, "a"), paragraph("para"), bullet(0, "b")]);
  normalized(markdown);
}

#[test]
fn docs_blocks_empty_items_keep_their_nested_items() {
  let markdown = "- - a\n  -\n- 1.\n  2. b\n";
  assert_eq!(
    kinds(markdown),
    vec![
      bullet(0, ""),
      bullet(1, "a"),
      bullet(1, ""),
      bullet(0, ""),
      ordered(1, 1, ""),
      ordered(2, 1, "b"),
    ]
  );
  normalized(markdown);
}

#[test]
fn docs_blocks_lists_that_cannot_continue_start_anew() {
  let written = round_trip(vec![
    bullet(0, "bullet"),
    ordered(1, 0, "then ordered"),
    ordered(7, 0, "a new ordered list"),
    bullet(0, "bullet again"),
    bullet(1, "nested"),
    ordered(5, 1, "nested from five"),
    bullet(0, "top"),
    ordered(5, 1, "from five right after text"),
    ordered(1, 0, ""),
    bullet(1, ""),
  ]);
  assert_eq!(
    written,
    "- bullet\n\n1. then ordered\n\n7) a new ordered list\n\n- bullet again\n  - nested\n\n  5. nested from five\n\
     - top\n\n  5. from five right after text\n\n1.\n   -\n"
  );
}

#[test]
fn docs_blocks_tasks_without_text_and_ordered_tasks_round_trip() {
  let written = round_trip(vec![
    task(Checkbox::Unchecked, 0, ""),
    task(Checkbox::Checked, 0, "done"),
    BlockKind::ListItem(ListItem {
      marker: ListMarker::Ordered(1),
      task: Some(Checkbox::Checked),
      depth: 0,
      text: plain("ordered task"),
    }),
  ]);
  assert_eq!(written, "- [ ]\n- [x] done\n\n1. [x] ordered task\n");
}

#[test]
fn docs_blocks_item_text_escapes_what_would_nest_a_block() {
  let written = round_trip(vec![
    bullet(0, "# not a heading"),
    bullet(0, "- not nested"),
    bullet(0, "1. not ordered"),
    bullet(0, "> not quoted"),
    bullet(0, "[ ] not a task"),
    bullet(0, "line\n- not an item"),
  ]);
  assert_eq!(
    written,
    "- \\# not a heading\n- \\- not nested\n- 1\\. not ordered\n- \\> not quoted\n- \\[ \\] not a task\n\
     - line\\\n  \\- not an item\n"
  );
}
