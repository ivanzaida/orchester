//! The text change of an `edit_file` call (`docs/architecture/native-agent.md` §4.4): its edits apply in
//! order to the text in memory, and the call fails as a whole when one of them does, so nothing is
//! written. Line endings and a byte order mark are the file's, not the model's: a file whose every line
//! ends in CRLF is edited as if it ended in LF and written back with CRLF, and a BOM stays in front.

use thiserror::Error;

use super::tools::Edit;

const BOM: char = '\u{feff}';

/// Why an edit cannot be applied. The message is the error result the model gets.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum EditError {
  #[error("edit {edit}: `old` was not found in the file; read it again and copy the text exactly")]
  NotFound { edit: usize },
  #[error("edit {edit}: `old` occurs {count} times; add surrounding lines to make it unique, or set replace_all")]
  Ambiguous { edit: usize, count: usize },
}

/// `text` with `edits` applied in order; `edit` numbers in errors count from 1.
pub fn apply_edits(text: &str, edits: &[Edit]) -> Result<String, EditError> {
  let (bom, body) = match text.strip_prefix(BOM) {
    Some(body) => (true, body),
    None => (false, text),
  };
  let crlf = only_crlf(body);
  let mut current = if crlf {
    body.replace("\r\n", "\n")
  } else {
    body.to_owned()
  };
  for (index, edit) in edits.iter().enumerate() {
    current = apply_one(&current, edit, index + 1, crlf)?;
  }
  if crlf {
    current = current.replace('\n', "\r\n");
  }
  if bom {
    current.insert(0, BOM);
  }
  Ok(current)
}

/// Applies `edit`; in a CRLF file (`crlf`) CRLF in the edit's text counts as LF, as the file is edited.
fn apply_one(text: &str, edit: &Edit, number: usize, crlf: bool) -> Result<String, EditError> {
  let lf = |value: &str| {
    if crlf {
      value.replace("\r\n", "\n")
    } else {
      value.to_owned()
    }
  };
  let (old, new) = (lf(edit.old()), lf(edit.new_text()));
  let count = text.matches(old.as_str()).count();
  match count {
    0 => Err(EditError::NotFound { edit: number }),
    1 => Ok(text.replacen(old.as_str(), &new, 1)),
    _ if edit.replace_all() => Ok(text.replace(old.as_str(), &new)),
    count => Err(EditError::Ambiguous { edit: number, count }),
  }
}

/// Whether `text` has line breaks and every one of them is CRLF.
fn only_crlf(text: &str) -> bool {
  let breaks = text.matches('\n').count();
  breaks > 0 && text.matches("\r\n").count() == breaks
}
