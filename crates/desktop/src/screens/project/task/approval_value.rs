//! A value the approval dialog asks about, shown in full (`bjrgu`, "Approval choices"): one that
//! fits on its row stays beside its label (`tfGRt`); a longer or multiline command, action or
//! target, and an MCP call's arguments always, move under their label into the transcript's command
//! box (`uA3Sh`) across the full width, lines as recorded, scrolling both ways inside a box no taller
//! than 12 lines (`E5NBo`); a quiet Copy sits at the right of the label's line (`ui::value_box`).

use lurq::{app::ctx::Ctx, components::Text, node::Element};
use orchester_core::run::ARGUMENTS_CEILING;

use super::approval_arguments::ArgumentsCache;
use crate::{
  theme::{APPROVAL_VALUE_MAX_HEIGHT, ExtraTypography},
  ui::{
    property::{ValueStyle, styled_property},
    value_box::{BoxText, CopyState, value_box},
  },
};

/// The most columns of mono 11 (Geist Mono advances 0.6 em, 6.6 px) a value takes beside its label
/// in the 600 px dialog: 544 px inside its padding, less the widest label (*Working directory*,
/// about 95 px) and the gap, leaves 441 px, room for 66. A few are kept in hand so that a value
/// which reaches the row's end is never cut with an ellipsis.
const INLINE_COLUMNS: usize = 60;

/// Builds the value rows of one approval; Copy records its outcome and hover in `copy`, and the arguments
/// are laid out once, in `arguments`.
pub(super) struct ValueRows<'a> {
  ctx: &'a Ctx,
  copy: &'a CopyState,
  arguments: &'a ArgumentsCache,
}

impl<'a> ValueRows<'a> {
  /// Ids of the rows: `{id}` is the box, `{id}-text` its lines, `{id}-scroll` where they scroll and
  /// `{id}-copy` its Copy.
  pub(super) const ACTION: &'static str = "tool-approval-action";
  pub(super) const TARGET: &'static str = "tool-approval-target";
  pub(super) const ARGUMENTS: &'static str = "tool-approval-arguments";

  pub(super) const fn new(ctx: &'a Ctx, copy: &'a CopyState, arguments: &'a ArgumentsCache) -> Self {
    Self { ctx, copy, arguments }
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
    self.boxed(id, label, value)
  }

  /// The command asked about beside *Command* when it fits there on one line, otherwise under a
  /// label that counts its lines (`nA5vu` in `Grcu7`), every line as recorded.
  pub(super) fn command(&self, command: &str) -> Element {
    let ctx = self.ctx;
    if fits_inline(command) {
      return styled_property(&ctx.t("runs.toolApproval.command"), command, ValueStyle::Mono);
    }
    // A trailing line break ends the last line; it does not start another one.
    let lines = command.trim_end_matches(['\r', '\n']).split('\n').count();
    let key = if lines == 1 {
      "runs.toolApproval.commandLines.one"
    } else {
      "runs.toolApproval.commandLines.other"
    };
    let label = ctx.t_args(key, [("lines", lines.to_string().as_str())]);
    self.boxed(Self::ACTION, &label, command)
  }

  /// The command in its box under `label`, which says why it asks (`J6guZs` in `NV6xJ`), however short
  /// it is.
  pub(super) fn labelled_command(&self, label: &str, command: &str) -> Element {
    self.boxed(Self::ACTION, label, command)
  }

  fn boxed(&self, id: &str, label: &str, value: &str) -> Element {
    let text = BoxText {
      shown: value,
      original: value,
    };
    value_box(self.ctx, (label, id), text, APPROVAL_VALUE_MAX_HEIGHT, self.copy).into()
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
    let text = BoxText {
      shown: &shown,
      original: recorded,
    };
    let note = laid_out.too_large.then(|| {
      Text::new(&self.ctx.t("runs.toolApproval.argumentsAsRecorded"))
        .id("tool-approval-arguments-note")
        .variant(ExtraTypography::Hint.role())
    });
    value_box(self.ctx, (label, id), text, APPROVAL_VALUE_MAX_HEIGHT, self.copy)
      .with_children(note)
      .into()
  }
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
