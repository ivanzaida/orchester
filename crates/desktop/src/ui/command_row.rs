//! Command row (`LBSz7` in 01.D, `s4DoJ` in 04.C1): a command in mono on the canvas fill with a
//! Copy button. Dialog and page rows show commands Orchester never runs; the user copies them into
//! a terminal. Transcript details show the command an agent ran, exactly as it was recorded, and the
//! approval dialog shows a long value an agent asks about in the same box.

use std::{borrow::Cow, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    events::MouseEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  clipboard::copy_to_clipboard,
  components::{Column, Row, ScrollBoth, ScrollHorizontal, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};

use super::{button::ActionButton, field::field_error, icon::Icon};
use crate::theme::{APPROVAL_VALUE_MAX_HEIGHT, ExtraColor, ExtraSpacing, ExtraTypography, command_scrollbar};

/// Copy button height (`eFsWL`).
const COPY_HEIGHT: f32 = 26.0;

/// The last command copied from a group of rows, and whether the clipboard took it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Copied(pub Option<(Arc<str>, bool)>);

impl lurq::app::component::DevtoolsInspectable for Copied {}

/// Where the row is: in a dialog (a form), with 11px text (`cdRcl`) and Copy as a Tab stop; on a
/// page (an inspector), with 10px text (`Z913A`) and Copy pointer-only, since only form controls
/// take the focus; or in a transcript entry's details, with the output's mono text (`mMUyR`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CommandPlace {
  Dialog,
  Page,
  /// Lines are kept as recorded and scroll sideways inside the box; Copy stays in view.
  Transcript,
  /// A value the approval dialog asks about: the transcript's box, with Copy a Tab stop of the
  /// dialog, and lines that also scroll down inside it once they are taller than
  /// [`APPROVAL_VALUE_MAX_HEIGHT`].
  Approval,
}

/// A single-line display preview: every run of whitespace that holds a line break, with the
/// indentation after it, reads as one space. Execution, details and copying use the original.
pub fn command_preview(command: &str) -> Cow<'_, str> {
  if !command.contains(['\r', '\n']) {
    return Cow::Borrowed(command);
  }
  let mut preview = String::with_capacity(command.len());
  let mut rest = command;
  while let Some(start) = rest.find(char::is_whitespace) {
    preview.push_str(&rest[..start]);
    let run = &rest[start..];
    let (space, tail) = run.split_at(run.find(|c: char| !c.is_whitespace()).unwrap_or(run.len()));
    if space.contains(['\r', '\n']) {
      preview.push(' ');
    } else {
      preview.push_str(space);
    }
    rest = tail;
  }
  preview.push_str(rest);
  Cow::Owned(preview)
}

/// A command row with ids `id`, `{id}-text` and `copy_id`; copying records the outcome in
/// `copied`. A transcript row also has `{id}-scroll`, the area its lines scroll in.
pub fn command_row(
  ctx: &Ctx,
  id: impl Into<Arc<str>>,
  copy_id: impl Into<Arc<str>>,
  command: &str,
  place: CommandPlace,
  copied: &Signal<Copied>,
) -> Element {
  laid_out_row(ctx, (id.into(), copy_id.into()), command, command, place, copied)
}

/// A command row that shows `shown`, `original` laid out for reading (e.g. JSON one member a
/// line), and copies `original`.
pub fn laid_out_row(
  ctx: &Ctx,
  (id, copy_id): (Arc<str>, Arc<str>),
  shown: &str,
  original: &str,
  place: CommandPlace,
  copied: &Signal<Copied>,
) -> Element {
  let outcome = match &copied.get().0 {
    Some((text, success)) if &**text == original => Some(*success),
    _ => None,
  };
  let label = match outcome {
    Some(true) => "common.copied",
    Some(false) | None => "common.copy",
  };
  let (signal, text) = (copied.clone(), Arc::<str>::from(original));
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
  let row = match place {
    CommandPlace::Dialog => wrapping_row(
      &id,
      shown,
      ExtraTypography::MonoValue.role(),
      copy.in_form().render(ctx),
    ),
    CommandPlace::Page => wrapping_row(&id, shown, TypographyStyle::Mono, copy.render(ctx)),
    CommandPlace::Transcript => transcript_row(&id, shown, copy.render(ctx), false),
    CommandPlace::Approval => transcript_row(&id, shown, copy.in_form().render(ctx), true),
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

/// The box around the command and Copy, with Copy at its top right.
fn command_box(id: &str) -> Row {
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding_right(ExtraSpacing::Tight.role())
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .background(PaletteColor::SurfaceBase)
    .border_inside(BorderSize::Sm, ExtraColor::Hairline.role())
    .rounded(RadiusSize::Md)
}

/// Dialog and page rows: the command wraps beside Copy; one line is centred on it by the text
/// column's minimum height.
fn wrapping_row(id: &str, command: &str, typography: TypographyStyle, copy: Element) -> Row {
  command_box(id)
    .padding_vertical(ExtraSpacing::Tight.role())
    .padding_left(ExtraSpacing::Inline.role())
    .child(
      Column::new()
        .min_width(Dimension::Px(0.0))
        .min_height(Dimension::Px(COPY_HEIGHT))
        .justify(Justify::Center)
        .flex(1.0)
        .child(
          Text::new(command)
            .id(format!("{id}-text"))
            .variant(typography)
            .color(PaletteColor::TextPrimary)
            .selectable(true)
            .min_width(Dimension::Px(0.0))
            .width(Dimension::Pct(100.0)),
        ),
    )
    .child(copy)
}

/// Transcript details (`uA3Sh` in `N4SlQq`): every line as recorded, never wrapped, scrolling
/// sideways in a viewport of the box's own; Copy keeps its column at the top right. The box's
/// bottom padding sits inside the viewport, where lurq overlays the horizontal thumb. A `capped`
/// viewport is no taller than [`APPROVAL_VALUE_MAX_HEIGHT`] and scrolls down as well.
fn transcript_row(id: &str, command: &str, copy: Element, capped: bool) -> Row {
  let lines = Column::new().padding_bottom(ExtraSpacing::Tight.role()).child(
    Column::new()
      .padding_vertical(ExtraSpacing::CommandLines.role())
      .padding_left(ExtraSpacing::Stack.role())
      .child(
        Text::new(command)
          .id(format!("{id}-text"))
          .variant(ExtraTypography::Output.role())
          .color(PaletteColor::TextPrimary)
          .selectable(true)
          .nowrap(),
      ),
  );
  command_box(id)
    .padding_top(ExtraSpacing::Tight.role())
    // Selecting or copying details must not toggle the enclosing transcript entry.
    .on_click(|event: MouseEvent| event.stop_propagation())
    .child(lines_viewport(id, lines, capped))
    .child(Column::new().padding_bottom(ExtraSpacing::Tight.role()).child(copy))
}

fn lines_viewport(id: &str, lines: Column, capped: bool) -> Element {
  let id = format!("{id}-scroll");
  if !capped {
    return ScrollHorizontal::new(lines)
      .id(id)
      .scrollbar(command_scrollbar())
      .min_width(Dimension::Px(0.0))
      .flex(1.0)
      .into();
  }
  ScrollBoth::new(lines)
    .id(id)
    .scrollbar(command_scrollbar())
    .min_width(Dimension::Px(0.0))
    .max_height(Dimension::Px(APPROVAL_VALUE_MAX_HEIGHT))
    .flex(1.0)
    .into()
}
