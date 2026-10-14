//! Shell variables a command line sets to a plain literal before it uses them (`f=src/lib.rs; sed -n
//! 1,20p $f`). Such a variable's uses are read as its value; anything else about variables stays
//! unread. A value is taken only when bash would keep it as written and its expansion cannot split into
//! other words or operators: no quote, space, `$`, backtick or shell operator in it. A backslash (a Windows
//! path, `L="$USERPROFILE/x"`) is kept in a value, which is then read only where it is used quoted: an
//! unquoted use is a glob, where a backslash escapes. A name is taken only from an allow-list no shell
//! treats specially: one letter, either case, with digits after it (`f`, `L`, `f2`). Every longer name could be special
//! or tied to one (zsh's `path` is `PATH`, `cdpath`, `fpath`, bash's `IFS`, `BASH_ENV`, `PS4`) or exported to programs
//! (`CC`, `http_proxy`, `GIT_DIR`, `LD_PRELOAD`), so its assignment stays unread. An assignment counts
//! only where it surely runs in the shell itself: first in its list (after the start of the line, a `;`
//! or a new line) and not in a pipeline or in the background.

/// Characters a literal value may hold, besides ASCII letters and digits.
const VALUE_CHARS: &str = "/._-:~+,@%=*?\\";

/// The variables set so far, with their values.
#[derive(Clone, Debug, Default)]
pub(super) struct Variables(Vec<(String, String)>);

impl Variables {
  /// The value of `name`, when a literal assignment set it.
  pub(super) fn value(&self, name: &str) -> Option<&str> {
    self
      .0
      .iter()
      .find_map(|(known, value)| (known == name).then_some(value.as_str()))
  }

  /// Takes `word`, the only word of a part that ran in the shell itself; `true` when it is a literal
  /// assignment whose later uses can be read. Any other assignment of a known name forgets it.
  pub(super) fn assign(&mut self, word: &str) -> bool {
    let Some((name, value)) = word.split_once('=') else {
      return false;
    };
    self.0.retain(|(known, _)| known != name);
    if !(plain_name(name) && literal(value)) {
      return false;
    }
    self.0.push((name.to_owned(), value.to_owned()));
    true
  }

  /// Forgets the variable an assignment word sets, when it is one.
  pub(super) fn forget(&mut self, word: &str) {
    if let Some((name, _)) = word.split_once('=') {
      self.0.retain(|(known, _)| known != name);
    }
  }
}

/// A name a later use may be read for: one letter and digits, which neither bash nor zsh treats
/// specially.
fn plain_name(name: &str) -> bool {
  let mut chars = name.chars();
  chars.next().is_some_and(|first| first.is_ascii_alphabetic()) && chars.all(|c| c.is_ascii_digit())
}

/// A value bash keeps as written whose expansion is the same words: not empty, no tilde after a `:`
/// (which bash expands in an assignment).
fn literal(value: &str) -> bool {
  !value.is_empty()
    && !value.contains(":~")
    && value
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || VALUE_CHARS.contains(c))
}

/// `NAME=value` sets a variable.
pub(super) fn is_assignment(word: &str) -> bool {
  word.split_once('=').is_some_and(|(name, _)| {
    name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && name.chars().all(name_char)
  })
}

/// Whether `c` may continue a variable name.
pub(super) const fn name_char(c: char) -> bool {
  c.is_ascii_alphanumeric() || c == '_'
}
