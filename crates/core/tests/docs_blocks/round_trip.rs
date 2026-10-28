use orchester_core::docs::blocks::{BlockKind, Body};

use super::support::normalized;

/// Real docs of this repository, as a person and agents write them.
const FIXTURES: [(&str, &str); 5] = [
  ("docs.md", include_str!("../../../../docs/design/docs.md")),
  ("code-viewer.md", include_str!("../../../../docs/design/code-viewer.md")),
  ("browser.md", include_str!("../../../../docs/design/browser.md")),
  (
    "orchestrator-toolset.md",
    include_str!("../../../../docs/design/orchestrator-toolset.md"),
  ),
  (
    "kontur-handover.md",
    include_str!("../../../../docs/design/kontur-handover.md"),
  ),
];

#[test]
fn docs_blocks_real_docs_round_trip() {
  for (name, markdown) in FIXTURES {
    let body = Body::parse(markdown);
    let written = body.to_markdown();
    assert_eq!(Body::parse(&written), body, "{name} written as:\n{written}");
    assert_eq!(
      Body::parse(&written).to_markdown(),
      written,
      "{name} is written differently the second time"
    );
  }
}

#[test]
fn docs_blocks_real_docs_keep_their_structure() {
  let body = Body::parse(FIXTURES[0].1);
  let count = |kind: fn(&BlockKind) -> bool| body.blocks().iter().filter(|block| kind(block.kind())).count();
  assert!(count(|kind| matches!(kind, BlockKind::Heading { level: 2, .. })) >= 10);
  assert!(count(|kind| matches!(kind, BlockKind::Table(_))) >= 3);
  assert!(count(|kind| matches!(kind, BlockKind::ListItem(_))) >= 30);
  let marked = body.blocks().iter().any(|block| match block.kind() {
    BlockKind::ListItem(item) => !item.text.marks().is_empty(),
    _ => false,
  });
  assert!(marked, "list items keep their marks");
}

#[test]
fn docs_blocks_markdown_is_written_in_its_normalized_form() {
  let markdown = "Title\n=====\n\
                  Some _emphasis_ and __strong__ with a [reference][r] and <https://example.com>.\n\
                  \n\
                  * one\n\
                  * two\n\
                  \n\
                  +   three\n\
                  \n\
                  |a|b|\n\
                  |-|:-|\n\
                  |   padded   |x|\n\
                  \n\
                  [r]: https://example.com/ref \"Title\"\n";
  assert_eq!(
    normalized(markdown),
    "# Title\n\nSome *emphasis* and **strong** with a [reference](https://example.com/ref) and \
     [https://example.com](https://example.com).\n\n- one\n- two\n- three\n\n| a | b |\n| --- | :-- |\n\
     | padded | x |\n"
  );
}
