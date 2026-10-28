use orchester_core::docs::blocks::{BlockKind, BlocksError, InlineText, Mark, MarkKind};

use super::support::{kinds, link, marked, normalized, paragraph, round_trip};

fn paragraph_text(markdown: &str) -> InlineText {
  match kinds(markdown).as_slice() {
    [BlockKind::Paragraph(text)] => text.clone(),
    other => panic!("one paragraph expected, got {other:?}"),
  }
}

#[test]
fn docs_blocks_marks_are_spans_over_the_text() {
  let markdown = "Some **bold**, *italic*, `code`, ~~gone~~ and [a link](https://example.com/a_b?x=1&y=2).\n";
  assert_eq!(
    paragraph_text(markdown),
    marked(
      "Some bold, italic, code, gone and a link.",
      &[
        (5, 9, MarkKind::Bold),
        (11, 17, MarkKind::Italic),
        (19, 23, MarkKind::Code),
        (25, 29, MarkKind::Strikethrough),
        (34, 40, link("https://example.com/a_b?x=1&y=2")),
      ]
    )
  );
  assert_eq!(normalized(markdown), markdown);
}

#[test]
fn docs_blocks_marks_inside_marks_keep_their_nesting() {
  let cases = [
    (
      "***both** italic*\n",
      marked("both italic", &[(0, 11, MarkKind::Italic), (0, 4, MarkKind::Bold)]),
    ),
    (
      "**bold *both* bold**\n",
      marked("bold both bold", &[(0, 14, MarkKind::Bold), (5, 9, MarkKind::Italic)]),
    ),
    (
      "[**b** `c` *d*](x)\n",
      marked(
        "b c d",
        &[
          (0, 5, link("x")),
          (0, 1, MarkKind::Bold),
          (2, 3, MarkKind::Code),
          (4, 5, MarkKind::Italic),
        ],
      ),
    ),
    (
      "**[in](x) bold**\n",
      marked("in bold", &[(0, 7, MarkKind::Bold), (0, 2, link("x"))]),
    ),
    (
      "~~*a* `b`~~\n",
      marked(
        "a b",
        &[
          (0, 3, MarkKind::Strikethrough),
          (0, 1, MarkKind::Italic),
          (2, 3, MarkKind::Code),
        ],
      ),
    ),
    (
      "**a***b*\n",
      marked("ab", &[(0, 1, MarkKind::Bold), (1, 2, MarkKind::Italic)]),
    ),
  ];
  for (markdown, expected) in cases {
    assert_eq!(paragraph_text(markdown), expected, "{markdown}");
    assert_eq!(normalized(markdown), markdown);
  }
}

#[test]
fn docs_blocks_marks_are_canonical() {
  let text = InlineText::new(
    "one two three",
    vec![
      Mark {
        start: 0,
        end: 3,
        kind: MarkKind::Bold,
      },
      Mark {
        start: 3,
        end: 7,
        kind: MarkKind::Bold,
      },
      Mark {
        start: 8,
        end: 8,
        kind: MarkKind::Italic,
      },
      Mark {
        start: 4,
        end: 13,
        kind: MarkKind::Code,
      },
      Mark {
        start: 2,
        end: 6,
        kind: MarkKind::Bold,
      },
    ],
  )
  .expect("marks inside the text");
  assert_eq!(
    text.marks(),
    [
      Mark {
        start: 0,
        end: 7,
        kind: MarkKind::Bold
      },
      Mark {
        start: 4,
        end: 13,
        kind: MarkKind::Code
      },
    ]
  );
  assert_eq!(
    InlineText::new(
      "ab",
      vec![Mark {
        start: 1,
        end: 3,
        kind: MarkKind::Bold
      }]
    ),
    Err(BlocksError::MarkOutsideText {
      start: 1,
      end: 3,
      len: 2
    })
  );
  assert_eq!(
    InlineText::new(
      "é",
      vec![Mark {
        start: 0,
        end: 1,
        kind: MarkKind::Bold
      }]
    ),
    Err(BlocksError::MarkOutsideText {
      start: 0,
      end: 1,
      len: 2
    })
  );
}

#[test]
fn docs_blocks_crossing_marks_are_written_closed_and_reopened() {
  let cases = [
    (
      marked("ab cd", &[(0, 4, MarkKind::Bold), (3, 5, MarkKind::Italic)]),
      "**ab *c***_d_\n",
    ),
    (
      marked("ab cd", &[(0, 4, MarkKind::Italic), (3, 5, MarkKind::Bold)]),
      "*ab **c***__d__\n",
    ),
    (
      marked("ab cd.", &[(0, 4, MarkKind::Bold), (3, 5, MarkKind::Italic)]),
      "**ab *c***_d_.\n",
    ),
    (
      marked("one two three", &[(0, 7, MarkKind::Bold), (4, 13, link("x"))]),
      "**one [two](x)**[ three](x)\n",
    ),
  ];
  for (text, expected) in cases {
    assert_eq!(round_trip(vec![BlockKind::Paragraph(text)]), expected);
  }
}

#[test]
fn docs_blocks_image_inside_a_link_is_its_alt_text_under_the_link() {
  assert_eq!(paragraph_text("[![b](i)](l)\n"), marked("b", &[(0, 1, link("l"))]));
  assert_eq!(normalized("[![b](i)](l)\n"), "[b](l)\n");
  assert_eq!(
    paragraph_text("See [![build](badge.svg)](ci) here\n"),
    marked("See build here", &[(4, 9, link("ci"))])
  );
  assert_eq!(
    normalized("See [![build](badge.svg)](ci) here\n"),
    "See [build](ci) here\n"
  );
  assert_eq!(
    paragraph_text("[see ![b](i) now](l)\n"),
    marked("see b now", &[(0, 9, link("l"))])
  );
  assert_eq!(normalized("[see ![b](i) now](l)\n"), "[see b now](l)\n");
  assert_eq!(normalized("- [![b](i)](l)\n"), "- [b](l)\n");
  assert_eq!(normalized("# [![b](i)](l)\n"), "# [b](l)\n");
  assert_eq!(
    normalized("| a |\n| --- |\n| [![b](i)](l) |\n"),
    "| a |\n| --- |\n| [b](l) |\n"
  );
}

#[test]
fn docs_blocks_marks_built_by_hand_round_trip() {
  let written = round_trip(vec![
    BlockKind::Paragraph(marked(
      "bold then code",
      &[(0, 4, MarkKind::Bold), (10, 14, MarkKind::Code)],
    )),
    BlockKind::Paragraph(marked("`ticks` and |", &[(0, 13, MarkKind::Code)])),
    BlockKind::Paragraph(marked(" spaced ", &[(0, 8, MarkKind::Code)])),
    BlockKind::Paragraph(marked(
      "to (parens) and spaces",
      &[(0, 2, link("a (b).md")), (3, 11, link(""))],
    )),
    BlockKind::Paragraph(marked("bang!link", &[(5, 9, link("x"))])),
  ]);
  assert_eq!(
    written,
    "**bold** then `code`\n\n`` `ticks` and | ``\n\n`  spaced  `\n\n[to](<a (b).md>) [(parens)](<>) and spaces\n\n\
     bang\\![link](x)\n"
  );
}

#[test]
fn docs_blocks_text_is_escaped_where_markdown_would_read_syntax() {
  let lines = [
    "# not a heading",
    "> not a quote",
    "- not an item",
    "+ not an item",
    "1. not ordered",
    "12) not ordered",
    "=== not a setext line",
    "*stars* _underscores_ snake_case [brackets] <tag> `ticks` ~tilde~ pipe | back\\slash",
    "&amp; stays as written, & alone too, &#35; and &#x23;",
    "    four spaces and a tab\t",
    "```",
    "---",
    "line\n# after a break\n- and an item",
  ];
  let body = lines.iter().map(|line| paragraph(line)).collect();
  let written = round_trip(body);
  assert_eq!(
    written,
    "\\# not a heading\n\n\\> not a quote\n\n\\- not an item\n\n\\+ not an item\n\n1\\. not ordered\n\n12\\) not ordered\n\n\
     \\=== not a setext line\n\n\
     \\*stars\\* \\_underscores\\_ snake_case \\[brackets\\] \\<tag> \\`ticks\\` \\~tilde\\~ pipe \\| back\\\\slash\n\n\
     \\&amp; stays as written, & alone too, \\&#35; and \\&#x23;\n\n\
     &#32;&#32;&#32;&#32;four spaces and a tab&#9;\n\n\\`\\`\\`\n\n\\---\n\n\
     line\\\n\\# after a break\\\n\\- and an item\n"
  );
}
