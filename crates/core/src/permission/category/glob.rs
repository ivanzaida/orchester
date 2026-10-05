//! What a word with a glob names: bash expands it to what it matches before the program sees it, and a
//! match may be a link that leads anywhere, so each match is placed, not the folder the glob is in.
//! Components are matched against what their folder holds now, case-sensitively as bash does: `*` and
//! `?`, bracket expressions (`[abc]`, `[a-z]`, `[!x]`), a leading `.` only by a pattern that starts with
//! one, which may also match `.` and `..` (bash before 5.2). `**`, which zsh expands through every
//! folder below, cannot be told. A folder that cannot be listed matches
//! nothing, as in bash; an entry that cannot be read, or more matches than [`MOST_MATCHES`], cannot be
//! told.

use std::path::{Component, Path, PathBuf};

/// The most matches followed; past them what a word names cannot be told.
const MOST_MATCHES: usize = 256;

/// The glob could not be followed: an entry could not be read, or it matches too much.
#[derive(Debug)]
pub(super) struct Untold;

/// The existing paths `pattern` (absolute) matches, as bash expands it; empty when it matches none, in
/// which case bash passes the word as written.
pub(super) fn expand(pattern: &Path) -> Result<Vec<PathBuf>, Untold> {
  let mut found = vec![PathBuf::new()];
  for component in pattern.components() {
    let name = match component {
      Component::Normal(name) => name.to_str().ok_or(Untold)?,
      other => {
        found.iter_mut().for_each(|path| path.push(other.as_os_str()));
        continue;
      }
    };
    if !name.contains(['*', '?', '[']) {
      found.iter_mut().for_each(|path| path.push(name));
      continue;
    }
    // zsh's `**` (and bash's with `globstar`) descends into every folder below, links included.
    if name.contains("**") {
      return Err(Untold);
    }
    let mut next = Vec::new();
    for folder in &found {
      for entry in entries(folder, name.starts_with('.'))? {
        if matches(name, &entry) {
          next.push(folder.join(entry));
        }
      }
      if next.len() > MOST_MATCHES {
        return Err(Untold);
      }
    }
    found = next;
  }
  Ok(found)
}

/// The names in `folder`, with `.` and `..` when the pattern starts with a dot; none when it is no
/// folder.
fn entries(folder: &Path, dots: bool) -> Result<Vec<String>, Untold> {
  // Bash matches nothing in a folder it cannot list (not there, no folder, not readable to this user,
  // a name the file system refuses) and passes the word as written; so does this.
  let Ok(listing) = std::fs::read_dir(folder) else {
    return Ok(Vec::new());
  };
  let mut names = if dots {
    vec![".".to_owned(), "..".to_owned()]
  } else {
    Vec::new()
  };
  for entry in listing {
    // An entry that cannot be read could be a match: what the word names cannot be told.
    let name = entry
      .map_err(|_| Untold)?
      .file_name()
      .into_string()
      .map_err(|_| Untold)?;
    if dots || !name.starts_with('.') {
      names.push(name);
    }
  }
  Ok(names)
}

/// Whether `name` matches `pattern`.
fn matches(pattern: &str, name: &str) -> bool {
  let pattern: Vec<char> = pattern.chars().collect();
  let name: Vec<char> = name.chars().collect();
  matches_from(&pattern, &name)
}

fn matches_from(pattern: &[char], name: &[char]) -> bool {
  match pattern.split_first() {
    None => name.is_empty(),
    Some(('*', rest)) => (0..=name.len()).any(|skip| matches_from(rest, &name[skip..])),
    Some(('?', rest)) => !name.is_empty() && matches_from(rest, &name[1..]),
    Some(('[', rest)) => match (bracket(rest), name.split_first()) {
      (Some((set, after)), Some((first, others))) => set.holds(*first) && matches_from(after, others),
      (Some(_), None) => false,
      // An unclosed bracket is a plain `[`.
      (None, _) => name.first() == Some(&'[') && matches_from(rest, &name[1..]),
    },
    Some((c, rest)) => name.first() == Some(c) && matches_from(rest, &name[1..]),
  }
}

/// A bracket expression's characters.
struct Bracket<'a> {
  negated: bool,
  items: &'a [char],
}

impl Bracket<'_> {
  fn holds(&self, c: char) -> bool {
    let mut held = false;
    let mut at = 0;
    while at < self.items.len() {
      let low = self.items[at];
      if self.items.get(at + 1) == Some(&'-') && at + 2 < self.items.len() {
        held |= (low..=self.items[at + 2]).contains(&c);
        at += 3;
      } else {
        held |= low == c;
        at += 1;
      }
    }
    held != self.negated
  }
}

/// The bracket expression `pattern` starts with (after its `[`), and the pattern after it; `None` when
/// it is not closed.
fn bracket(pattern: &[char]) -> Option<(Bracket<'_>, &[char])> {
  let negated = matches!(pattern.first(), Some('!' | '^'));
  let start = usize::from(negated);
  // The expression holds at least one character, so a `]` first is one of them, not its end.
  let close = start + 1 + pattern.get(start + 1..)?.iter().position(|c| *c == ']')?;
  Some((
    Bracket {
      negated,
      items: &pattern[start..close],
    },
    &pattern[close + 1..],
  ))
}
