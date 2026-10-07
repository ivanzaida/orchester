//! `apply_edits`: edits in order, all or none, with the file's line endings and BOM kept, and the lines they
//! changed.

use orchester_core::native::{
  edit::{EditError, LineChanges, apply_edits},
  tools::{Edit, ToolInput, parse_call},
};
use serde_json::json;

fn edits(pairs: &[(&str, &str, bool)]) -> Vec<Edit> {
  let edits: Vec<_> = pairs
    .iter()
    .map(|(old, new, all)| json!({"old": old, "new": new, "replace_all": all}))
    .collect();
  match parse_call("edit_file", &json!({"path": "a.txt", "edits": edits})) {
    Ok(ToolInput::EditFile(input)) => input.edits().to_vec(),
    other => panic!("{other:?}"),
  }
}

#[test]
fn native_edit_applies_in_order() {
  let changed = text(apply_edits("a b c", &edits(&[("a", "x", false), ("x b", "y", false)])));
  assert_eq!(changed, Ok("y c".to_owned()));
}

#[test]
fn native_edit_names_the_failing_edit() {
  assert_eq!(
    text(apply_edits("a b a", &edits(&[("b", "c", false), ("a", "d", false)]))),
    Err(EditError::Ambiguous { edit: 2, count: 2 })
  );
  assert_eq!(
    text(apply_edits("a", &edits(&[("z", "y", false)]))),
    Err(EditError::NotFound { edit: 1 })
  );
  assert_eq!(
    text(apply_edits("a b a", &edits(&[("a", "d", true)]))),
    Ok("d b d".to_owned())
  );
}

#[test]
fn native_edit_keeps_line_endings_and_bom() {
  let crlf = "\u{feff}one\r\ntwo\r\n";
  assert_eq!(
    text(apply_edits(crlf, &edits(&[("one\ntwo", "1\n2\n3", false)]))),
    Ok("\u{feff}1\r\n2\r\n3\r\n".to_owned())
  );
  assert_eq!(
    text(apply_edits(crlf, &edits(&[("one\r\ntwo", "1\r\n2", false)]))),
    Ok("\u{feff}1\r\n2\r\n".to_owned())
  );
  let mixed = "one\r\ntwo\nthree\n";
  assert_eq!(
    text(apply_edits(mixed, &edits(&[("two\nthree", "2\n3", false)]))),
    Ok("one\r\n2\n3\n".to_owned())
  );
  assert_eq!(
    text(apply_edits(mixed, &edits(&[("one\ntwo", "1", false)]))),
    Err(EditError::NotFound { edit: 1 })
  );
}

fn text(edited: Result<orchester_core::native::edit::Edited, EditError>) -> Result<String, EditError> {
  edited.map(|edited| edited.text)
}

#[test]
fn native_edit_counts_changed_lines() {
  let lines = |text: &str, pairs: &[(&str, &str, bool)]| apply_edits(text, &edits(pairs)).map(|edited| edited.lines);
  let changes = |added, removed| Ok(LineChanges { added, removed });
  assert_eq!(lines("a\nb\nc\n", &[("b\n", "B\nB2\n", false)]), changes(2, 1));
  assert_eq!(lines("a\nb\n", &[("a\n", "a\nnew\n", false)]), changes(1, 0));
  assert_eq!(lines("x y x\n", &[("x", "z", true)]), changes(2, 2));
  assert_eq!(
    lines(
      "one\r\ntwo\r\n",
      &[("one\r\ntwo", "one\r\n2", false), ("2", "3", false)]
    ),
    changes(2, 2)
  );
  assert_eq!(LineChanges::between("", "a\nb\n"), LineChanges { added: 2, removed: 0 });
}
