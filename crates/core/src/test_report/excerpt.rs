//! Report text made fit for an agent to read: terminal colour codes removed, long text capped at a character
//! boundary, and a failure's excerpt of a few lines.

/// `text` without terminal escape sequences (the colours vitest and Playwright put in their messages).
pub(super) fn plain(text: &str) -> String {
  let mut plain = String::with_capacity(text.len());
  let mut chars = text.chars().peekable();
  while let Some(c) = chars.next() {
    if c != '\u{1b}' {
      plain.push(c);
      continue;
    }
    match chars.next() {
      // CSI: parameters, then one final byte in `@`..=`~`.
      Some('[') => {
        for c in chars.by_ref() {
          if ('@'..='~').contains(&c) {
            break;
          }
        }
      }
      // OSC (hyperlinks): ends with BEL or ESC `\`.
      Some(']') => {
        while let Some(c) = chars.next() {
          if c == '\u{7}' || (c == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
            break;
          }
        }
      }
      _ => {}
    }
  }
  plain
}

/// The first `max_bytes` of `text` and how many bytes were left out.
pub(super) fn head(text: &str, max_bytes: usize) -> (&str, usize) {
  let end = text.floor_char_boundary(max_bytes);
  (&text[..end], text.len() - end)
}

/// The last `max_bytes` of `text` and how many bytes were left out before them.
pub(super) fn tail(text: &str, max_bytes: usize) -> (&str, usize) {
  let start = text.ceil_char_boundary(text.len().saturating_sub(max_bytes));
  (&text[start..], start)
}

/// At most `max_lines` of `lines`, each cut to `max_chars` characters.
pub(super) fn lines<'a>(lines: impl Iterator<Item = &'a str>, max_lines: usize, max_chars: usize) -> String {
  lines
    .take(max_lines)
    .map(|line| match line.char_indices().nth(max_chars) {
      Some((end, _)) => format!("{}…", &line[..end]),
      None => line.to_owned(),
    })
    .collect::<Vec<_>>()
    .join("\n")
}
