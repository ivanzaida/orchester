//! The text change of an `edit_file` call (`docs/architecture/native-agent.md` §4.4): its edits apply in
//! order to the text in memory, and the call fails as a whole when one of them does, so nothing is
//! written. Line endings and a byte order mark are the file's, not the model's: a file whose every line
//! ends in CRLF is edited as if it ended in LF and written back with CRLF, and a BOM stays in front. How
//! many lines a change adds and removes is counted for its transcript row (04.H10, *+38 −6*).

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

/// Lines a change added and removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LineChanges {
  pub added: u64,
  pub removed: u64,
}

impl LineChanges {
  /// What replacing `old` with `new` changes: the lines between what they share at their start and
  /// at their end.
  pub fn between(old: &str, new: &str) -> Self {
    let old: Vec<&str> = old.split_inclusive('\n').collect();
    let new: Vec<&str> = new.split_inclusive('\n').collect();
    let (start, end) = shared_ends(&old, &new);
    let count = |lines: usize| u64::try_from(lines).unwrap_or(u64::MAX);
    Self {
      added: count(new.len() - start - end),
      removed: count(old.len() - start - end),
    }
  }

  fn times(self, count: usize) -> Self {
    let count = u64::try_from(count).unwrap_or(u64::MAX);
    Self {
      added: self.added.saturating_mul(count),
      removed: self.removed.saturating_mul(count),
    }
  }

  fn plus(self, other: Self) -> Self {
    Self {
      added: self.added.saturating_add(other.added),
      removed: self.removed.saturating_add(other.removed),
    }
  }
}

/// How many lines `old` and `new` share at their start, and then at their end: what lies between changed.
pub fn shared_ends<T: PartialEq>(old: &[T], new: &[T]) -> (usize, usize) {
  let start = old.iter().zip(new).take_while(|(a, b)| a == b).count();
  let end = old[start..]
    .iter()
    .rev()
    .zip(new[start..].iter().rev())
    .take_while(|(a, b)| a == b)
    .count();
  (start, end)
}

/// A text with edits applied, and the lines they changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edited {
  pub text: String,
  pub lines: LineChanges,
}

/// `text` with `edits` applied in order; `edit` numbers in errors count from 1.
pub fn apply_edits(text: &str, edits: &[Edit]) -> Result<Edited, EditError> {
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
  let mut lines = LineChanges::default();
  for (index, edit) in edits.iter().enumerate() {
    let (changed, counted) = apply_one(&current, edit, index + 1, crlf)?;
    current = changed;
    lines = lines.plus(counted);
  }
  if crlf {
    current = current.replace('\n', "\r\n");
  }
  if bom {
    current.insert(0, BOM);
  }
  Ok(Edited { text: current, lines })
}

/// Applies `edit`; in a CRLF file (`crlf`) CRLF in the edit's text counts as LF, as the file is edited.
fn apply_one(text: &str, edit: &Edit, number: usize, crlf: bool) -> Result<(String, LineChanges), EditError> {
  let lf = |value: &str| {
    if crlf {
      value.replace("\r\n", "\n")
    } else {
      value.to_owned()
    }
  };
  let (old, new) = (lf(edit.old()), lf(edit.new_text()));
  let lines = LineChanges::between(&old, &new);
  let count = text.matches(old.as_str()).count();
  match count {
    0 => Err(EditError::NotFound { edit: number }),
    1 => Ok((text.replacen(old.as_str(), &new, 1), lines)),
    _ if edit.replace_all() => Ok((text.replace(old.as_str(), &new), lines.times(count))),
    count => Err(EditError::Ambiguous { edit: number, count }),
  }
}

/// Whether `text` has line breaks and every one of them is CRLF.
fn only_crlf(text: &str) -> bool {
  let breaks = text.matches('\n').count();
  breaks > 0 && text.matches("\r\n").count() == breaks
}
