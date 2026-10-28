use orchester_core::docs::blocks::{BlockKind, BlocksError, Body};

use super::support::paragraph;

fn ids(body: &Body) -> Vec<u64> {
  body.blocks().iter().map(|block| block.id().get()).collect()
}

#[test]
fn docs_blocks_ids_are_numbered_in_order_by_a_parse() {
  let body = Body::parse("# Title\n\nText\n\n- item\n");
  assert_eq!(ids(&body), [0, 1, 2]);
  assert_eq!(Body::parse(&body.to_markdown()), body);
}

#[test]
fn docs_blocks_ids_stay_with_their_blocks_across_edits() {
  let mut body = Body::parse("one\n\ntwo\n\nthree\n");
  let two = body.blocks()[1].id();
  let three = body.blocks()[2].id();

  let new = body.insert(1, paragraph("between")).expect("inside the body");
  assert_eq!(ids(&body), [0, 3, 1, 2]);
  assert_eq!(body.block(two).map(|block| block.kind()), Some(&paragraph("two")));

  assert_eq!(body.replace(two, paragraph("TWO")), Ok(paragraph("two")));
  assert_eq!(body.blocks()[2].id(), two);

  assert_eq!(body.remove(new), Ok(paragraph("between")));
  let added = body.insert(3, BlockKind::Divider).expect("at the end");
  assert_eq!(added.get(), 4, "an id is never given twice in one body");
  assert_eq!(ids(&body), [0, 1, 2, 4]);
  assert_eq!(body.block(three).map(|block| block.kind()), Some(&paragraph("three")));
  assert_eq!(body.to_markdown(), "one\n\nTWO\n\nthree\n\n---\n");
}

#[test]
fn docs_blocks_edits_of_missing_blocks_are_refused() {
  let mut body = Body::parse("one\n");
  let id = body.blocks()[0].id();
  assert_eq!(body.remove(id), Ok(paragraph("one")));
  assert_eq!(body.remove(id), Err(BlocksError::UnknownBlock(id)));
  assert_eq!(body.replace(id, BlockKind::Divider), Err(BlocksError::UnknownBlock(id)));
  assert_eq!(
    body.insert(1, BlockKind::Divider),
    Err(BlocksError::OutOfRange { index: 1, len: 0 })
  );
  assert_eq!(body.to_markdown(), "");
}
