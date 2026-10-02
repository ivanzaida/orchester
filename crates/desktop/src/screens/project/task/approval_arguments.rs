//! An MCP call's arguments laid out for reading in the approval dialog: JSON one member a line,
//! indented two spaces a level. The agent writes the arguments, so the layout is bounded: it is
//! made once per approval, and arguments that would grow too large laid out are shown as recorded.

use std::sync::Arc;

use orchester_core::run::cut_arguments;
use parking_lot::Mutex;
use serde::de::IgnoredAny;

/// Laid out, arguments may grow to this many times their recorded size…
const LAID_OUT_GROWTH: usize = 4;
/// …and always to this many bytes, so small but deeply nested arguments are still laid out…
const LAID_OUT_FLOOR: usize = 64 * 1024;
/// …but never past this many bytes, whatever their recorded size.
const LAID_OUT_LIMIT: usize = 512 * 1024;

/// What the Arguments box shows of recorded arguments, before the line that says they were cut.
pub(super) struct LaidOutArguments {
  /// JSON laid out, or the arguments as recorded.
  pub(super) text: Arc<str>,
  /// They are shown as recorded because laid out they would be too large.
  pub(super) too_large: bool,
  /// They were cut when recorded; `text` is the part kept.
  pub(super) cut: bool,
}

impl LaidOutArguments {
  fn of(recorded: &str) -> Self {
    let (source, cut) = match cut_arguments(recorded) {
      Some(kept) => (kept, true),
      None => (recorded, false),
    };
    let as_recorded = |too_large| Self {
      text: source.into(),
      too_large,
      cut,
    };
    // What does not parse, such as arguments an older version cut when it recorded them, is shown
    // as recorded. The part kept before a cut cannot parse, but is the start of JSON.
    if !cut && serde_json::from_str::<IgnoredAny>(source).is_err() {
      return as_recorded(false);
    }
    let limit = (source.len().saturating_mul(LAID_OUT_GROWTH)).clamp(LAID_OUT_FLOOR, LAID_OUT_LIMIT);
    match lay_out_json(source, limit) {
      Some(text) => Self {
        text: text.into(),
        too_large: false,
        cut,
      },
      None => as_recorded(true),
    }
  }
}

/// The arguments of the approval shown, laid out once: the dialog renders again on every Copy and
/// while a decision is sent, and each render reuses them.
#[derive(Default)]
pub(super) struct ArgumentsCache(Mutex<Option<CachedArguments>>);

struct CachedArguments {
  approval: String,
  recorded: Arc<str>,
  laid_out: Arc<LaidOutArguments>,
}

impl ArgumentsCache {
  /// `recorded`, the arguments of `approval`, laid out.
  pub(super) fn get(&self, approval: &str, recorded: &str) -> Arc<LaidOutArguments> {
    let mut cache = self.0.lock();
    if let Some(cached) = &*cache
      && cached.approval == approval
      && *cached.recorded == *recorded
    {
      return cached.laid_out.clone();
    }
    let laid_out = Arc::new(LaidOutArguments::of(recorded));
    *cache = Some(CachedArguments {
      approval: approval.to_owned(),
      recorded: recorded.into(),
      laid_out: laid_out.clone(),
    });
    laid_out
  }
}

/// `raw` one member or element a line, indented two spaces a level, a space after each colon, or
/// `None` once that grows past `limit` bytes. Only the whitespace between tokens changes, so every
/// key, string and number reads exactly as recorded and in its recorded order. `raw` need not be
/// whole: the part of JSON before a cut is laid out as far as it goes.
fn lay_out_json(raw: &str, limit: usize) -> Option<String> {
  let mut layout = JsonLayout::default();
  let mut chars = raw.chars().peekable();
  while let Some(c) = chars.next() {
    if layout.text.len() > limit {
      return None;
    }
    if layout.in_string {
      layout.string_char(c);
      continue;
    }
    match c {
      '{' | '[' => {
        while chars.next_if(|next| next.is_whitespace()).is_some() {}
        match chars.next_if(|next| matches!(next, '}' | ']')) {
          Some(close) => layout.text.extend([c, close]),
          None => layout.open(c),
        }
      }
      '}' | ']' => layout.close(c),
      ',' => {
        layout.text.push(c);
        layout.new_line();
      }
      ':' => layout.text.push_str(": "),
      '"' => {
        layout.in_string = true;
        layout.text.push(c);
      }
      c if c.is_whitespace() => {}
      c => layout.text.push(c),
    }
  }
  (layout.text.len() <= limit).then_some(layout.text)
}

/// The text [`lay_out_json`] writes, and where it is in it.
#[derive(Default)]
struct JsonLayout {
  text: String,
  depth: usize,
  in_string: bool,
  escaped: bool,
}

impl JsonLayout {
  fn string_char(&mut self, c: char) {
    self.text.push(c);
    match (self.escaped, c) {
      (true, _) => self.escaped = false,
      (false, '\\') => self.escaped = true,
      (false, '"') => self.in_string = false,
      (false, _) => {}
    }
  }

  fn open(&mut self, c: char) {
    self.text.push(c);
    self.depth += 1;
    self.new_line();
  }

  fn close(&mut self, c: char) {
    self.depth = self.depth.saturating_sub(1);
    self.new_line();
    self.text.push(c);
  }

  fn new_line(&mut self) {
    self.text.push('\n');
    self.text.extend(std::iter::repeat_n(' ', self.depth * 2));
  }
}
