//! Command row (`LBSz7` in 01.D, `s4DoJ` in 04.C1): a command in mono on the canvas fill with a
//! Copy button. Orchester never runs these commands; the user copies them into a terminal.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
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
    CommandPlace::Dialog => (copy, ExtraTypography::MonoValue.role()),
    CommandPlace::Page => (copy.pointer_only(), TypographyStyle::Mono),
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
        .variant(typography)
        .color(PaletteColor::TextPrimary)
        .selectable(true)
        .flex(1.0),
    )
    .child(copy);
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
