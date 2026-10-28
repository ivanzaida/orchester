//! A block's inline text as Markdown: marks as delimiters, nested as they open and close, and the text escaped where
//! Markdown would read it as syntax.

use std::{cmp::Reverse, ops::Range};

use super::{InlineText, Mark, MarkKind};

/// Where inline text is written, which decides how its line breaks and edges are written.
#[derive(Clone, Copy)]
pub(super) enum Flow<'a> {
  /// Text that may break lines: a hard break is `\` at the end of a line, and the next line starts with `prefix`.
  Lines { prefix: &'a str },
  /// A heading: one line, and `#` at its end escaped so it is not a closing sequence.
  Heading,
  /// A table cell: one line, and `|` escaped in code too.
  Cell,
}

/// A mark open in the Markdown written, with the character its emphasis is written with (`*` or `_`).
#[derive(Clone, Copy)]
struct Open<'m> {
  mark: &'m Mark,
  emphasis: char,
}

/// Writes `inline` to `out`.
pub(super) fn inline(out: &mut String, inline: &InlineText, flow: Flow<'_>) {
  let text = inline.text();
  let marks = inline.marks();
  let mut bounds: Vec<usize> = marks.iter().flat_map(|mark| [mark.start, mark.end]).collect();
  bounds.extend([0, text.len()]);
  bounds.sort_unstable();
  bounds.dedup();
  let mut open: Vec<Open<'_>> = Vec::new();
  let mut starting = marks.iter().peekable();
  for (index, &at) in bounds.iter().enumerate() {
    let mut opening: Vec<(&Mark, bool)> = close_at(out, text, &mut open, at)
      .into_iter()
      .map(|mark| (mark, true))
      .collect();
    while let Some(mark) = starting.next_if(|mark| mark.start == at) {
      opening.push((mark, false));
    }
    // The mark that lasts longest is outermost; code is innermost, as nothing can be inside it.
    opening.sort_by_key(|(mark, _)| (Reverse(mark.end), mark.kind == MarkKind::Code));
    for (mark, reopened) in opening {
      // A mark crossing the edge of one that ended goes on right after its closing `*`: with `*` too, the two
      // would be one run of delimiters, read differently.
      let emphasis = if reopened && out.ends_with('*') && underscore_fits(text, at..mark.end) {
        '_'
      } else {
        '*'
      };
      let mark = Open { mark, emphasis };
      open_mark(out, text, mark);
      open.push(mark);
    }
    let Some(&end) = bounds.get(index + 1) else {
      break;
    };
    if open.iter().any(|open| open.mark.kind == MarkKind::Code) {
      code_text(out, &text[at..end], flow);
    } else {
      escaped(out, text, at..end, flow);
    }
  }
}

/// Writes `text` as plain text, as an image's alt text is.
pub(super) fn plain(out: &mut String, text: &str, flow: Flow<'_>) {
  escaped(out, text, 0..text.len(), flow);
}

/// Whether `_` emphasis over `rest` of `text` is read as such: no space at its edges, and no letter or digit right
/// after it.
fn underscore_fits(text: &str, rest: Range<usize>) -> bool {
  let inner = &text[rest.clone()];
  inner.chars().next().is_some_and(|first| !first.is_whitespace())
    && inner.chars().next_back().is_some_and(|last| !last.is_whitespace())
    && !text[rest.end..].chars().next().is_some_and(char::is_alphanumeric)
}

/// Closes the marks ending at `at`, and every mark opened inside them, which go on in the returned marks.
fn close_at<'m>(out: &mut String, text: &str, open: &mut Vec<Open<'m>>, at: usize) -> Vec<&'m Mark> {
  let Some(first) = open.iter().position(|open| open.mark.end <= at) else {
    return Vec::new();
  };
  for &mark in open[first..].iter().rev() {
    close_mark(out, text, mark);
  }
  open
    .split_off(first)
    .into_iter()
    .map(|open| open.mark)
    .filter(|mark| mark.end > at)
    .collect()
}

fn open_mark(out: &mut String, text: &str, Open { mark, emphasis }: Open<'_>) {
  match &mark.kind {
    MarkKind::Bold => out.extend([emphasis, emphasis]),
    MarkKind::Italic => out.push(emphasis),
    MarkKind::Strikethrough => out.push_str("~~"),
    MarkKind::Link(_) => out.push('['),
    MarkKind::Code => {
      let (fence, pad) = code_fence(&text[mark.start..mark.end]);
      out.push_str(&fence);
      out.push_str(pad);
    }
  }
}

fn close_mark(out: &mut String, text: &str, Open { mark, emphasis }: Open<'_>) {
  match &mark.kind {
    MarkKind::Bold => out.extend([emphasis, emphasis]),
    MarkKind::Italic => out.push(emphasis),
    MarkKind::Strikethrough => out.push_str("~~"),
    MarkKind::Link(target) => {
      out.push_str("](");
      destination(out, target);
      out.push(')');
    }
    MarkKind::Code => {
      let (fence, pad) = code_fence(&text[mark.start..mark.end]);
      out.push_str(pad);
      out.push_str(&fence);
    }
  }
}

/// The backticks around a code span holding `code`, and the space inside them that keeps backticks and spaces at
/// its edges as they are.
fn code_fence(code: &str) -> (String, &'static str) {
  let runs = backtick_runs(code);
  let length = (1..).find(|length| !runs.contains(length)).unwrap_or(1);
  let padded = code.starts_with('`')
    || code.ends_with('`')
    || (code.starts_with(' ') && code.ends_with(' ') && !code.chars().all(|ch| ch == ' '));
  ("`".repeat(length), if padded { " " } else { "" })
}

/// The lengths of the runs of backticks in `text`.
fn backtick_runs(text: &str) -> Vec<usize> {
  let mut runs = Vec::new();
  let mut run = 0;
  for ch in text.chars().chain([' ']) {
    if ch == '`' {
      run += 1;
    } else if run > 0 {
      runs.push(run);
      run = 0;
    }
  }
  runs
}

fn code_text(out: &mut String, code: &str, flow: Flow<'_>) {
  for ch in code.chars() {
    if ch == '|' && matches!(flow, Flow::Cell) {
      out.push('\\');
    }
    out.push(ch);
  }
}

/// A link's target: bare with `\`, parentheses and entities escaped, or between `<` and `>` when it is empty or holds
/// spaces or angle brackets.
pub(super) fn destination(out: &mut String, target: &str) {
  let pointed = target.is_empty()
    || target
      .chars()
      .any(|ch| ch.is_whitespace() || ch.is_control() || "<>".contains(ch));
  let special = if pointed { "<>\\" } else { "()\\" };
  if pointed {
    out.push('<');
  }
  for (at, ch) in target.char_indices() {
    if special.contains(ch) || (ch == '&' && starts_entity(&target[at + 1..])) {
      out.push('\\');
    }
    out.push(ch);
  }
  if pointed {
    out.push('>');
  }
}

/// Writes `run` of `text` escaped. What lies around the run in `text` decides where its lines start and end.
fn escaped(out: &mut String, text: &str, run: Range<usize>, flow: Flow<'_>) {
  for (at, ch) in text[run.clone()].char_indices().map(|(at, ch)| (run.start + at, ch)) {
    if ch == '\n' {
      match flow {
        Flow::Lines { prefix } => {
          out.push_str("\\\n");
          out.push_str(prefix);
        }
        Flow::Heading | Flow::Cell => out.push(' '),
      }
      continue;
    }
    if (ch == ' ' || ch == '\t') && at_line_edge(text, at) {
      out.push_str(if ch == ' ' { "&#32;" } else { "&#9;" });
      continue;
    }
    if needs_escape(text, &run, at, ch, flow) {
      out.push('\\');
    }
    out.push(ch);
  }
}

fn needs_escape(text: &str, run: &Range<usize>, at: usize, ch: char, flow: Flow<'_>) -> bool {
  match ch {
    '\\' | '`' | '*' | '[' | ']' | '<' | '~' | '|' => true,
    '_' => {
      let before = text[run.start..at].chars().next_back();
      let after = text[at + 1..run.end].chars().next();
      !(before.is_some_and(char::is_alphanumeric) && after.is_some_and(char::is_alphanumeric))
    }
    '&' => starts_entity(&text[at + 1..]),
    // Before a link's `[` it would make the link an image.
    '!' => at + 1 == run.end,
    '#' if matches!(flow, Flow::Heading) && text[at..].chars().all(|ch| ch == '#') => true,
    '#' | '>' | '-' | '+' | '=' | ':' => line_start(text, at),
    '.' | ')' => ordered_marker(text, at),
    _ => false,
  }
}

/// Whether what follows `&` makes it a character reference (`&amp;`, `&#38;`, `&#x26;`), so that `&` would not be
/// read as itself.
fn starts_entity(rest: &str) -> bool {
  let Some((name, _)) = rest.split_once(';') else {
    return false;
  };
  let (digits, hex) = match name.strip_prefix('#') {
    Some(number) => match number.strip_prefix(['x', 'X']) {
      Some(hex) => (hex, true),
      None => (number, false),
    },
    None => return !name.is_empty() && name.len() <= 32 && name.chars().all(|ch| ch.is_ascii_alphanumeric()),
  };
  !digits.is_empty()
    && digits.len() <= 7
    && digits.chars().all(|ch| {
      if hex {
        ch.is_ascii_hexdigit()
      } else {
        ch.is_ascii_digit()
      }
    })
}

fn line_start(text: &str, at: usize) -> bool {
  text[..at].chars().next_back().is_none_or(|before| before == '\n')
}

/// Whether the `.` or `)` at `at` follows a line's first one to nine digits, which would make an ordered list item.
fn ordered_marker(text: &str, at: usize) -> bool {
  let digits = text[..at].chars().rev().take_while(char::is_ascii_digit).count();
  (1..=9).contains(&digits) && line_start(text, at - digits)
}

/// Whether the space or tab at `at` is among the blanks at the start or end of its line, which Markdown drops.
fn at_line_edge(text: &str, at: usize) -> bool {
  let blank = |ch: &char| *ch == ' ' || *ch == '\t';
  let before = text[..at].chars().rev().find(|ch| !blank(ch));
  let after = text[at..].chars().find(|ch| !blank(ch));
  matches!(before, None | Some('\n')) || matches!(after, None | Some('\n'))
}
