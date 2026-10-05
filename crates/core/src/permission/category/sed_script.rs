//! Reading a `sed` script to its end, the way GNU and BSD `sed` agree on it, to tell that it neither
//! writes a file, reads one, nor runs a command. Only what both read alike is taken: a regular
//! expression with a bracket expression is not (BSD skips a delimiter inside one, GNU does not), nor
//! a command other than those listed in [`command_end`].

/// Whether `script` only prints, selects, changes or adds lines: the commands of [`command_end`], each
/// after up to two addresses and an optional `!`, separated by `;` or new lines, in `{…}` groups.
pub(super) fn harmless(script: &str) -> bool {
  let mut rest = script;
  let mut depth = 0usize;
  loop {
    rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == ';');
    let Some(first) = rest.chars().next() else {
      return depth == 0;
    };
    if first == '}' {
      let Some(outer) = depth.checked_sub(1) else {
        return false;
      };
      depth = outer;
      rest = &rest[1..];
      continue;
    }
    let Some(after) = addresses(rest) else {
      return false;
    };
    let after = after.trim_start();
    let after = after.strip_prefix('!').unwrap_or(after).trim_start();
    let Some(command) = after.chars().next() else {
      return false;
    };
    let Some(next) = command_end(command, &after[command.len_utf8()..], &mut depth) else {
      return false;
    };
    if !(next.is_empty() || next.starts_with(|c: char| c.is_whitespace() || c == ';' || c == '}')) {
      return false;
    }
    rest = next;
  }
}

/// What follows `command` and its arguments in `rest`: `{` opens a group; `p`, `P`, `=`, `d`, `D`, `n`,
/// `N`, `g`, `G`, `h`, `H`, `x` and `z` take nothing; `q` and `Q` an exit code; `a`, `i` and `c` their
/// text to the end of the line; `s` a substitution without the `w` and `e` flags; `y` its two lists.
/// `None` for any other command (`w`, `W`, `r`, `R`, `e`, `b`, …) or one not read to its end.
fn command_end<'a>(command: char, rest: &'a str, depth: &mut usize) -> Option<&'a str> {
  match command {
    '{' => {
      *depth += 1;
      Some(rest)
    }
    'p' | 'P' | '=' | 'd' | 'D' | 'n' | 'N' | 'g' | 'G' | 'h' | 'H' | 'x' | 'z' => Some(rest),
    'q' | 'Q' => Some(rest.trim_start_matches(|c: char| c == ' ' || c.is_ascii_digit())),
    'a' | 'i' | 'c' => Some(rest.find('\n').map_or("", |at| &rest[at..])),
    's' => {
      let (sections, after) = delimited(rest, 2)?;
      if has_bracket(sections[0]) {
        return None;
      }
      Some(after.trim_start_matches(|c: char| "gpiImM".contains(c) || c.is_ascii_digit()))
    }
    'y' => delimited(rest, 2).map(|(_, after)| after),
    _ => None,
  }
}

/// The `count` sections of `text` after its first character, the delimiter, each ended by the
/// delimiter unescaped, and what follows them.
fn delimited(text: &str, count: usize) -> Option<(Vec<&str>, &str)> {
  let mut chars = text.char_indices();
  let (_, delimiter) = chars.next()?;
  if matches!(delimiter, '\\' | '\n') {
    return None;
  }
  let mut sections = Vec::with_capacity(count);
  let mut start = delimiter.len_utf8();
  while sections.len() < count {
    let (at, c) = chars.next()?;
    if c == '\\' {
      chars.next()?;
    } else if c == delimiter {
      sections.push(&text[start..at]);
      start = at + c.len_utf8();
    }
  }
  Some((sections, &text[start..]))
}

/// `text` after the addresses it starts with (none, one, or two joined by a comma); `None` when they
/// cannot be read. An address is a line number, `first~step`, `$`, `+n` or `~n` after a comma, or a
/// `/regex/` (with `I` or `M`).
fn addresses(text: &str) -> Option<&str> {
  let rest = address(text)?;
  match rest.strip_prefix(',') {
    Some(second) => {
      let after = match second.strip_prefix(['+', '~']) {
        Some(count) => digits(count),
        None => address(second)?,
      };
      (after.len() < second.len()).then_some(after)
    }
    None => Some(rest),
  }
}

/// `text` after the one address it may start with.
fn address(text: &str) -> Option<&str> {
  if let Some(rest) = text.strip_prefix('$') {
    return Some(rest);
  }
  if text.starts_with('/') {
    let (sections, rest) = delimited(text, 1)?;
    if has_bracket(sections[0]) {
      return None;
    }
    return Some(rest.trim_start_matches(['I', 'M']));
  }
  let rest = digits(text);
  Some(rest.strip_prefix('~').map_or(rest, digits))
}

/// Whether `regex` holds a bracket expression: a `[` not escaped.
fn has_bracket(regex: &str) -> bool {
  let mut chars = regex.chars();
  while let Some(c) = chars.next() {
    match c {
      '\\' => {
        chars.next();
      }
      '[' => return true,
      _ => {}
    }
  }
  false
}

fn digits(text: &str) -> &str {
  text.trim_start_matches(|c: char| c.is_ascii_digit())
}
