//! `for NAME in w1 w2 …; do BODY; done`, the whole line: the body runs once for each listed literal, so
//! the line does what `NAME=w1; BODY` then `NAME=w2; BODY` … do, and is read as those lines. The name is
//! one a literal assignment may set ([`super::variables`]), the words are plain literals, at most
//! [`MOST_ROUNDS`] of them. Any other loop (`while`, `until`, a `for` over `$(…)` or a glob, a C-style
//! `for`, one inside a longer line) is not read.

/// The most rounds a loop is read for.
const MOST_ROUNDS: usize = 64;

/// What `text` is: no loop (`None`), or a loop and the lines its rounds run (`Some(None)` when it
/// cannot be read).
pub(super) fn rounds(text: &str) -> Option<Option<Vec<String>>> {
  let rest = text.trim().strip_prefix("for ")?;
  Some(read(rest))
}

fn read(rest: &str) -> Option<Vec<String>> {
  let (header, rest) = rest.split_once(';').or_else(|| rest.split_once('\n'))?;
  let mut words = header.split_whitespace();
  let name = words.next()?;
  if words.next()? != "in" {
    return None;
  }
  let values: Vec<&str> = words.collect();
  let plain_name = name.chars().next().is_some_and(|first| first.is_ascii_alphabetic())
    && name.chars().skip(1).all(|c| c.is_ascii_digit());
  let plain_values = values.iter().all(|value| {
    value
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || "._-:,=+@%/".contains(c))
  });
  if !plain_name || !plain_values || values.is_empty() || values.len() > MOST_ROUNDS {
    return None;
  }
  let body = rest.trim_start().strip_prefix("do")?;
  if !body.starts_with([' ', '\n', '\t']) {
    return None;
  }
  // `done` closes the loop only as a command of its own, after a `;` or a new line.
  let body = body.trim_end().strip_suffix("done")?.trim_end_matches([' ', '\t']);
  let body = body.strip_suffix(';').or_else(|| body.strip_suffix('\n'))?.trim();
  if body.is_empty() {
    return None;
  }
  Some(values.iter().map(|value| format!("{name}={value}; {body}")).collect())
}
