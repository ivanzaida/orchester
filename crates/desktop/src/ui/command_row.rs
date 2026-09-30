//! Command row (`LBSz7` in 01.D, `s4DoJ` in 04.C1): a command in mono on the canvas fill with a
//! Copy button. Transcript details show the original command recorded by the agent.

use std::{borrow::Cow, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    events::MouseEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  clipboard::copy_to_clipboard,
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{button::ActionButton, field::field_error, icon::Icon};
use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography};

/// Copy button height (`eFsWL`).
const COPY_HEIGHT: f32 = 26.0;

/// The last command copied from a group of rows, and whether the clipboard took it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Copied(pub Option<(Arc<str>, bool)>);

impl lurq::app::component::DevtoolsInspectable for Copied {}

/// Where the row is: in a dialog (a form), with 11px text (`cdRcl`) and Copy as a Tab stop, or on
/// a page (an inspector), with 10px text (`Z913A`) and Copy pointer-only, since only form
/// controls take the focus.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CommandPlace {
  Dialog,
  Page,
  /// Original transcript command, wrapping in the transcript's scrolling viewport.
  Transcript,
}

/// A single-line display preview; execution, details and copying use the original command.
pub fn command_preview(command: &str) -> Cow<'_, str> {
  if !command.contains(['\r', '\n']) {
    return Cow::Borrowed(command);
  }
  Cow::Owned(command.replace("\r\n", "\n").replace(['\r', '\n'], " "))
}

/// A command row with ids `{id}` and `{id}-copy`; copying records the outcome in `copied`.
pub fn command_row(
  ctx: &Ctx,
  id: &'static str,
  copy_id: &'static str,
  command: &str,
  place: CommandPlace,
  copied: &Signal<Copied>,
) -> Element {
  let outcome = match &copied.get().0 {
    Some((text, success)) if &**text == command => Some(*success),
    _ => None,
  };
  let label = match outcome {
    Some(true) => "common.copied",
    Some(false) | None => "common.copy",
  };
  let (signal, text) = (copied.clone(), Arc::<str>::from(command));
  let copy = ActionButton::new(ctx.t(label))
    .icon(if outcome == Some(true) { Icon::Check } else { Icon::Copy })
    .compact()
    .id(copy_id)
    .height(Dimension::Px(COPY_HEIGHT))
    .on_click(move || {
      let success = copy_to_clipboard(&*text);
      if !success {
        tracing::warn!("the clipboard did not take the command");
      }
      signal.set(Copied(Some((text.clone(), success))));
    });
  let (copy, typography) = match place {
    CommandPlace::Dialog => (copy.in_form(), ExtraTypography::MonoValue.role()),
    CommandPlace::Page => (copy, TypographyStyle::Mono),
    CommandPlace::Transcript => (copy, ExtraTypography::Output.role()),
  };
  let copy = copy.render(ctx);
  let row = Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Tight.role())
    .padding_right(ExtraSpacing::Tight.role())
    .padding_left(ExtraSpacing::Inline.role())
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceBase)
    .border_inside(BorderSize::Sm, ExtraColor::Hairline.role())
    .rounded(RadiusSize::Md)
    .child(
      Text::new(command)
        .id(format!("{id}-text"))
        .variant(typography)
        .color(PaletteColor::TextPrimary)
        .selectable(true)
        .min_width(Dimension::Px(0.0))
        .flex(1.0),
    )
    .child(copy);
  // Selecting or copying details must not toggle the enclosing transcript entry.
  let row = if matches!(place, CommandPlace::Transcript) {
    row.on_click(|event: MouseEvent| event.stop_propagation())
  } else {
    row
  };
  if outcome != Some(false) {
    return row.into();
  }
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(row)
    .child(field_error(&ctx.t("common.copyFailed")))
    .into()
}
