//! A value the approval dialog asks about, shown in full (`bjrgu`, "Approval choices"): one that
//! fits on its row stays beside its label (`tfGRt`); a longer or multiline command, action or
//! target, and an MCP call's arguments always, move under their label into the transcript's command
//! box (`uA3Sh`), lines as recorded, scrolling both ways inside a box no taller than 12 lines
//! (`E5NBo`), with Copy at its top right.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::run::ARGUMENTS_CEILING;

use super::approval_arguments::ArgumentsCache;
use crate::{
  theme::{APPROVAL_VALUE_MAX_HEIGHT, ExtraTypography},
  ui::{
    command_row::{CommandPlace, Copied, command_row, laid_out_row},
    property::{ValueStyle, styled_property},
  },
};

/// The most columns of mono 11 (Geist Mono advances 0.6 em, 6.6 px) a value takes beside its label
/// in the 600 px dialog: 544 px inside its padding, less the widest label (*Working directory*,
/// about 95 px) and the gap, leaves 441 px, room for 66. A few are kept in hand so that a value
/// which reaches the row's end is never cut with an ellipsis.
const INLINE_COLUMNS: usize = 60;

/// Where a long value goes: the transcript's box, capped as the design caps it.
const PLACE: CommandPlace = CommandPlace::Capped {
  max_height: APPROVAL_VALUE_MAX_HEIGHT,
};

/// Builds the value rows of one approval; Copy records its outcome in `copied`, and the arguments
/// are laid out once, in `arguments`.
pub(super) struct ValueRows<'a> {
  ctx: &'a Ctx,
  copied: &'a Signal<Copied>,
  arguments: &'a ArgumentsCache,
}

impl<'a> ValueRows<'a> {
  /// Ids of the rows: `{id}` is the box, `{id}-text` its lines, `{id}-scroll` where they scroll and
  /// `{id}-copy` its Copy.
  pub(super) const ACTION: &'static str = "tool-approval-action";
  pub(super) const TARGET: &'static str = "tool-approval-target";
  pub(super) const ARGUMENTS: &'static str = "tool-approval-arguments";

  pub(super) const fn new(ctx: &'a Ctx, copied: &'a Signal<Copied>, arguments: &'a ArgumentsCache) -> Self {
    Self { ctx, copied, arguments }
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
    let row = command_row(self.ctx, id, format!("{id}-copy"), value, PLACE, self.copied);
    block(label, row, None)
  }

  /// The arguments of `approval` under `label`: JSON one member a line when they parse, otherwise
  /// as recorded. Arguments cut when they were recorded end with a line that says so; arguments too
  /// large to lay out are shown as recorded, with a note that says so. Copy takes them as recorded.
  pub(super) fn arguments(&self, label: &str, approval: &str, recorded: &str) -> Element {
    let id = Self::ARGUMENTS;
    let laid_out = self.arguments.get(approval, recorded);
    let cut_line = laid_out.cut.then(|| {
      let kilobytes = (ARGUMENTS_CEILING / 1024).to_string();
      self
        .ctx
        .t_args("runs.toolApproval.argumentsCut", [("kilobytes", kilobytes.as_str())])
    });
    let shown = match &cut_line {
      Some(cut) => format!("{}\n{cut}", laid_out.text),
      None => laid_out.text.to_string(),
    };
    let row = laid_out_row(
      self.ctx,
      (id.into(), format!("{id}-copy").into()),
      &shown,
      recorded,
      PLACE,
      self.copied,
    );
    let note = laid_out
      .too_large
      .then(|| self.ctx.t("runs.toolApproval.argumentsAsRecorded"));
    block(label, row, note.as_deref())
  }
}

/// The label above its value's box, as the shell string's (`bwWuT`), and a note under it.
fn block(label: &str, row: Element, note: Option<&str>) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(label).variant(TypographyStyle::Caption))
    .child(row)
    .with_children(note.map(|note| {
      Text::new(note)
        .id("tool-approval-arguments-note")
        .variant(ExtraTypography::Hint.role())
    }))
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
