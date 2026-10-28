use orchester_core::docs::blocks::{BlockKind, ColumnAlign, MarkKind, Table};

use super::support::{kinds, link, marked, normalized, paragraph, plain, round_trip};

#[test]
fn docs_blocks_headings_keep_their_level_and_are_written_atx() {
  let markdown = "# One\n\nTwo\n===\n\nThree\n---\n\n###### Six ##\n\n#\n";
  assert_eq!(
    kinds(markdown),
    vec![
      BlockKind::Heading {
        level: 1,
        text: plain("One")
      },
      BlockKind::Heading {
        level: 1,
        text: plain("Two")
      },
      BlockKind::Heading {
        level: 2,
        text: plain("Three")
      },
      BlockKind::Heading {
        level: 6,
        text: plain("Six")
      },
      BlockKind::Heading {
        level: 1,
        text: plain("")
      },
    ]
  );
  assert_eq!(normalized(markdown), "# One\n\n# Two\n\n## Three\n\n###### Six\n\n#\n");
}

#[test]
fn docs_blocks_paragraphs_join_soft_breaks_and_keep_hard_ones() {
  let markdown = "First line\nsame paragraph  \nafter a hard break\\\nand another\n\nSecond\n";
  assert_eq!(
    kinds(markdown),
    vec![
      paragraph("First line same paragraph\nafter a hard break\nand another"),
      paragraph("Second"),
    ]
  );
  assert_eq!(
    normalized(markdown),
    "First line same paragraph\\\nafter a hard break\\\nand another\n\nSecond\n"
  );
}

#[test]
fn docs_blocks_quotes_hold_one_paragraph_each() {
  let markdown = "> first\n> still first\n>\n> second\n\n>\n\n> > nested\n";
  assert_eq!(
    kinds(markdown),
    vec![
      BlockKind::Quote(plain("first still first")),
      BlockKind::Quote(plain("second")),
      BlockKind::Quote(plain("")),
      BlockKind::Quote(plain("nested")),
    ]
  );
  assert_eq!(
    normalized(markdown),
    "> first still first\n\n> second\n\n>\n\n> nested\n"
  );
}

#[test]
fn docs_blocks_code_blocks_keep_their_language_and_text() {
  let markdown = "```rust\nfn main() {}\n\n```\n\n    indented\n\n~~~\n```\ninside\n```\n~~~\n\n```\n```\n";
  assert_eq!(
    kinds(markdown),
    vec![
      BlockKind::Code {
        language: "rust".to_owned(),
        text: "fn main() {}\n".to_owned()
      },
      BlockKind::Code {
        language: String::new(),
        text: "indented".to_owned()
      },
      BlockKind::Code {
        language: String::new(),
        text: "```\ninside\n```".to_owned()
      },
      BlockKind::Code {
        language: String::new(),
        text: String::new()
      },
    ]
  );
  assert_eq!(
    normalized(markdown),
    "```rust\nfn main() {}\n\n```\n\n```\nindented\n```\n\n````\n```\ninside\n```\n````\n\n```\n```\n"
  );
}

#[test]
fn docs_blocks_tables_keep_cells_marks_and_alignment() {
  let markdown = "| Name | Size | Note |\n|:-----|-----:|:----:|\n| `a\\|b` | **2** | [x](y) |\n| short |\n";
  let expected = Table {
    columns: vec![ColumnAlign::Left, ColumnAlign::Right, ColumnAlign::Center],
    header: vec![plain("Name"), plain("Size"), plain("Note")],
    rows: vec![
      vec![
        marked("a|b", &[(0, 3, MarkKind::Code)]),
        marked("2", &[(0, 1, MarkKind::Bold)]),
        marked("x", &[(0, 1, link("y"))]),
      ],
      vec![plain("short"), plain(""), plain("")],
    ],
  };
  assert_eq!(kinds(markdown), vec![BlockKind::Table(expected)]);
  assert_eq!(
    normalized(markdown),
    "| Name | Size | Note |\n| :-- | --: | :-: |\n| `a\\|b` | **2** | [x](y) |\n| short |  |  |\n"
  );
}

#[test]
fn docs_blocks_images_alone_are_blocks_and_among_text_are_links() {
  let markdown = "![Alt *text*](images/a.png \"Title\")\n\n![one](1.png) ![two](2.png)\n\nSee ![icon](i.png) here\n";
  assert_eq!(
    kinds(markdown),
    vec![
      BlockKind::Image {
        alt: "Alt text".to_owned(),
        target: "images/a.png".to_owned()
      },
      BlockKind::Image {
        alt: "one".to_owned(),
        target: "1.png".to_owned()
      },
      BlockKind::Image {
        alt: "two".to_owned(),
        target: "2.png".to_owned()
      },
      BlockKind::Paragraph(marked("See icon here", &[(4, 8, link("i.png"))])),
    ]
  );
  assert_eq!(
    normalized(markdown),
    "![Alt text](images/a.png)\n\n![one](1.png)\n\n![two](2.png)\n\nSee [icon](i.png) here\n"
  );
}

#[test]
fn docs_blocks_dividers_are_written_as_three_dashes() {
  let markdown = "a\n\n***\n\n_ _ _\n";
  assert_eq!(
    kinds(markdown),
    vec![paragraph("a"), BlockKind::Divider, BlockKind::Divider]
  );
  assert_eq!(normalized(markdown), "a\n\n---\n\n---\n");
}

#[test]
fn docs_blocks_raw_html_is_kept_as_literal_text() {
  let markdown = "<div>\n  <b>x</b>\n</div>\n\nText with <span>inline</span> HTML.\n";
  assert_eq!(
    kinds(markdown),
    vec![
      paragraph("<div>\n  <b>x</b>\n</div>"),
      paragraph("Text with <span>inline</span> HTML."),
    ]
  );
  normalized(markdown);
}

#[test]
fn docs_blocks_every_kind_built_round_trips() {
  let table = Table {
    columns: vec![ColumnAlign::Auto, ColumnAlign::Center],
    header: vec![plain("A"), marked("B c", &[(0, 1, MarkKind::Italic)])],
    rows: vec![vec![plain(""), plain("x | y")]],
  };
  let written = round_trip(vec![
    BlockKind::Heading {
      level: 3,
      text: plain("Title #"),
    },
    paragraph("Body text"),
    super::support::bullet(0, "item"),
    BlockKind::Quote(plain("quoted\nline")),
    BlockKind::Code {
      language: "toml".to_owned(),
      text: "a = 1".to_owned(),
    },
    BlockKind::Table(table),
    BlockKind::Image {
      alt: "a [b]".to_owned(),
      target: "with space.png".to_owned(),
    },
    BlockKind::Divider,
  ]);
  assert_eq!(
    written,
    "### Title \\#\n\nBody text\n\n- item\n\n> quoted\\\n> line\n\n```toml\na = 1\n```\n\n| A | *B* c |\n| --- | :-: |\n\
     |  | x \\| y |\n\n![a \\[b\\]](<with space.png>)\n\n---\n"
  );
}
