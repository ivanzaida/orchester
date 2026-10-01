//! Codex's standard error as a step's activity keeps it. App Server writes its own diagnostics there,
//! some of which echo the user's configuration: a warning about settings Codex does not know names
//! each setting and its file on indented lines under it. Those lines are left out, colours are
//! removed, and an invocation keeps at most [`MAX_LINES`] lines of at most [`MAX_LINE_CHARS`]
//! characters, [`MAX_BYTES`] in all; what comes after is dropped. Secret values are redacted later,
//! with every other activity of the step.

/// Lines of standard error kept per invocation.
const MAX_LINES: usize = 1000;
/// Bytes of standard error kept per invocation.
const MAX_BYTES: usize = 256 * 1024;
/// Characters kept of one line.
const MAX_LINE_CHARS: usize = 4000;
/// Starts Codex's warning about configuration settings it ignores, which names them.
const SETTINGS_ECHO: &str = "unrecognized configuration setting";

/// What an invocation has kept of Codex's standard error so far.
#[derive(Default)]
pub(super) struct StderrLog {
  lines: usize,
  bytes: usize,
  /// The previous line started Codex's echo of the user's settings.
  in_echo: bool,
}

impl StderrLog {
  /// `line` as the activity keeps it, or `None` when it is left out.
  pub(super) fn keep(&mut self, line: &str) -> Option<String> {
    let line = without_colours(line);
    if self.in_echo && line.starts_with(' ') {
      return None;
    }
    self.in_echo = line.contains(SETTINGS_ECHO);
    if self.in_echo || self.lines >= MAX_LINES || self.bytes >= MAX_BYTES {
      return None;
    }
    let kept: String = line.chars().take(MAX_LINE_CHARS).collect();
    self.lines += 1;
    self.bytes += kept.len();
    Some(kept)
  }
}

/// `line` without ANSI escape sequences (`ESC [ … letter`), which colour Codex's log lines.
fn without_colours(line: &str) -> String {
  let mut plain = String::with_capacity(line.len());
  let mut chars = line.chars();
  while let Some(c) = chars.next() {
    if c == '\u{1b}' {
      if chars.next() == Some('[') {
        for c in chars.by_ref() {
          if c.is_ascii_alphabetic() {
            break;
          }
        }
      }
    } else {
      plain.push(c);
    }
  }
  plain
}
