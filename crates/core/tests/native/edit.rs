//! `apply_edits`: edits in order, all or none, with the file's line endings and BOM kept.

use orchester_core::native::{
  edit::{EditError, apply_edits},
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
  let changed = apply_edits("a b c", &edits(&[("a", "x", false), ("x b", "y", false)]));
  assert_eq!(changed, Ok("y c".to_owned()));
}

#[test]
fn native_edit_names_the_failing_edit() {
  assert_eq!(
    apply_edits("a b a", &edits(&[("b", "c", false), ("a", "d", false)])),
    Err(EditError::Ambiguous { edit: 2, count: 2 })
  );
  assert_eq!(
    apply_edits("a", &edits(&[("z", "y", false)])),
    Err(EditError::NotFound { edit: 1 })
  );
  assert_eq!(
    apply_edits("a b a", &edits(&[("a", "d", true)])),
    Ok("d b d".to_owned())
  );
}

#[test]
fn native_edit_keeps_line_endings_and_bom() {
  let crlf = "\u{feff}one\r\ntwo\r\n";
  assert_eq!(
    apply_edits(crlf, &edits(&[("one\ntwo", "1\n2\n3", false)])),
    Ok("\u{feff}1\r\n2\r\n3\r\n".to_owned())
  );
  assert_eq!(
    apply_edits(crlf, &edits(&[("one\r\ntwo", "1\r\n2", false)])),
    Ok("\u{feff}1\r\n2\r\n".to_owned())
  );
  let mixed = "one\r\ntwo\nthree\n";
  assert_eq!(
    apply_edits(mixed, &edits(&[("two\nthree", "2\n3", false)])),
    Ok("one\r\n2\n3\n".to_owned())
  );
  assert_eq!(
    apply_edits(mixed, &edits(&[("one\ntwo", "1", false)])),
    Err(EditError::NotFound { edit: 1 })
  );
}
