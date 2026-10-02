//! A value the approval dialog asks about, shown in full: one that fits on its row stays beside its
//! label (`tfGRt`); a longer or multiline command, action or target, and an MCP call's arguments
//! always, move under their label into the transcript's command box (`uA3Sh`), lines as recorded,
//! scrolling both ways inside it, with Copy at its top right. The design draws no long value yet;
//! the block follows the full shell string of 04.H9 (`bwWuT` in `coKjY`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};

use crate::ui::{
  command_row::{CommandPlace, Copied, command_row, laid_out_row},
  property::{ValueStyle, styled_property},
};

/// The most columns of mono 11 (Geist Mono advances 0.6 em, 6.6 px) a value takes beside its label
/// in the 600 px dialog: 544 px inside its padding, less the widest label (*Working directory*,
/// about 95 px) and the gap, leaves 441 px, room for 66. A few are kept in hand so that a value
/// which reaches the row's end is never cut with an ellipsis.
const INLINE_COLUMNS: usize = 60;

/// Builds the value rows of one approval; Copy records its outcome in `copied`.
pub(super) struct ValueRows<'a> {
  ctx: &'a Ctx,
  copied: &'a Signal<Copied>,
}

impl<'a> ValueRows<'a> {
  /// Ids of the rows: `{id}` is the box, `{id}-text` its lines, `{id}-scroll` where they scroll and
  /// `{id}-copy` its Copy.
  pub(super) const ACTION: &'static str = "tool-approval-action";
  pub(super) const TARGET: &'static str = "tool-approval-target";
  pub(super) const ARGUMENTS: &'static str = "tool-approval-arguments";

  pub(super) const fn new(ctx: &'a Ctx, copied: &'a Signal<Copied>) -> Self {
    Self { ctx, copied }
  }

  pub(super) const fn ctx(&self) -> &'a Ctx {
    self.ctx
  }

  /// `value` beside `label` when it fits there on one line, otherwise under it, every line as
  /// recorded.
  pub(super) fn value(&self, id: &str, label: &str, value: &str) -> Element {
    if fits_inline(value) {
      return styled_property(label, value, ValueStyle::Mono);
    }
    let row = command_row(
      self.ctx,
      id,
      format!("{id}-copy"),
      value,
      CommandPlace::Approval,
      self.copied,
    );
    block(label, row)
  }

  /// An MCP call's arguments under `label`: JSON one member a line when they parse, otherwise as
  /// recorded (they are cut when longer than the runner records). Copy takes them as recorded.
  pub(super) fn arguments(&self, label: &str, arguments: &str) -> Element {
    let id = Self::ARGUMENTS;
    let readable = readable_json(arguments);
    let row = laid_out_row(
      self.ctx,
      (id.into(), format!("{id}-copy").into()),
      readable.as_deref().unwrap_or(arguments),
      arguments,
      CommandPlace::Approval,
      self.copied,
    );
    block(label, row)
  }
}

/// The label above its value's box, as the shell string's (`bwWuT`).
fn block(label: &str, row: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(label).variant(TypographyStyle::Caption))
    .child(row)
    .into()
}

/// One line that fits beside the label. A character outside ASCII counts twice: the mono face has
/// no glyph for most of them, and the fallback's are wider.
fn fits_inline(value: &str) -> bool {
  if value.chars().any(char::is_control) {
    return false;
  }
  let columns: usize = value.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum();
  columns <= INLINE_COLUMNS
}

/// `raw` laid out for reading when it is JSON: one member or element a line, indented two spaces a
/// level, a space after each colon. Only the whitespace between tokens changes, so every key,
/// string and number reads exactly as recorded and in its recorded order.
fn readable_json(raw: &str) -> Option<String> {
  // What does not parse, such as arguments cut when they were recorded, is shown as recorded.
  serde_json::from_str::<serde_json::Value>(raw).ok()?;
  let mut layout = JsonLayout::default();
  let mut chars = raw.chars().peekable();
  while let Some(c) = chars.next() {
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
  Some(layout.text)
}

/// The text [`readable_json`] writes, and where it is in it.
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
