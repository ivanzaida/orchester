//! `edit_file`'s line edit: lines change only when they are exactly what the call expects, else the call is refused
//! with what they hold now; the file's line endings, byte order mark and missing last line ending are kept.

use orchester_core::orchester_tools::{FileCall, LineEdit, LineEditError, ToolCall, range_now, replace_lines};
use serde_json::json;

use crate::parse;

fn edit(start: u32, end: u32, expected: &str, content: &str) -> LineEdit {
  let arguments =
    json!({"filename": "a.txt", "start_line": start, "end_line": end, "expected": expected, "content": content});
  match parse("edit_file", &arguments) {
    Ok(ToolCall::File(call)) => match *call {
      FileCall::Edit(edit) => edit,
      other => panic!("an edit, not {other:?}"),
    },
    other => panic!("a valid edit, not {other:?}"),
  }
}

fn applied(text: &str, edit: &LineEdit) -> String {
  replace_lines(text, edit).expect("applies").text
}

#[test]
fn orchester_tools_edit_replaces_exactly_the_expected_lines() {
  let text = "one\ntwo\nthree\nfour\n";
  let changed = replace_lines(text, &edit(2, 3, "two\nthree", "2\n3\n3.5")).expect("applies");
  assert_eq!(changed.text, "one\n2\n3\n3.5\nfour\n");
  assert_eq!((changed.lines.added, changed.lines.removed), (3, 2));
  // A trailing line break of `expected` or `content` is no line of its own.
  assert_eq!(applied(text, &edit(1, 1, "one\n", "ONE\n")), "ONE\ntwo\nthree\nfour\n");
  // Empty content removes the lines.
  assert_eq!(applied(text, &edit(2, 3, "two\nthree", "")), "one\nfour\n");
  // end_line = start_line - 1 inserts before start_line, also after the last line and into an empty file.
  assert_eq!(applied(text, &edit(1, 0, "", "zero")), "zero\none\ntwo\nthree\nfour\n");
  assert_eq!(applied(text, &edit(5, 4, "", "five")), "one\ntwo\nthree\nfour\nfive\n");
  assert_eq!(applied("", &edit(1, 0, "", "first")), "first\n");
  // A text that is only a line ending is one blank line: it can be expected, replaced and inserted.
  let blank = "one\n\nthree\n";
  assert_eq!(applied(blank, &edit(2, 2, "\n", "two\n")), "one\ntwo\nthree\n");
  assert_eq!(applied(blank, &edit(2, 2, "\r\n", "")), "one\nthree\n");
  assert_eq!(applied(text, &edit(2, 1, "", "\n")), "one\n\ntwo\nthree\nfour\n");
}

/// A stale `expected` changes nothing: the answer names the range and what it holds now.
#[test]
fn orchester_tools_edit_refuses_stale_expected_lines() {
  let text = "one\ntwo\nthree\n";
  assert_eq!(
    replace_lines(text, &edit(2, 2, "TWO", "2")),
    Err(LineEditError::Changed {
      start: 2,
      end: 2,
      now: "     2\ttwo\n".to_owned()
    })
  );
  let refused = replace_lines(text, &edit(1, 2, "one", "1")).expect_err("one line short");
  assert!(refused.to_string().contains("     1\tone\n     2\ttwo\n"), "{refused}");
  assert_eq!(
    replace_lines(text, &edit(3, 4, "three\nfour", "x")),
    Err(LineEditError::PastEnd {
      lines: 3,
      start: 3,
      end: 4
    })
  );
  assert!(replace_lines(text, &edit(5, 4, "", "x")).is_err());
  assert_eq!(range_now(text, 2, 9), "     2\ttwo\n     3\tthree\n");
  assert_eq!(range_now(text, 4, 3), "(no line: the range is empty)\n");
}

/// CRLF, a byte order mark and a last line without its ending stay as the file had them.
#[test]
fn orchester_tools_edit_keeps_the_files_line_endings() {
  assert_eq!(
    applied("\u{feff}a\r\nb\r\nc\r\n", &edit(2, 2, "b", "B\nB2")),
    "\u{feff}a\r\nB\r\nB2\r\nc\r\n"
  );
  assert_eq!(applied("a\r\nb\r\n", &edit(1, 1, "a\r\n", "A\r\n")), "A\r\nb\r\n");
  assert_eq!(applied("a\nb", &edit(2, 2, "b", "B\nC")), "a\nB\nC");
  assert_eq!(applied("a\nb", &edit(3, 2, "", "c")), "a\nb\nc");
  assert_eq!(applied("a\nb\n", &edit(1, 1, "a", "A")), "A\nb\n");
}
