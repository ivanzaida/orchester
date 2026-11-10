//! What the compact picker has at its foot besides the segmented efforts: for a connection that lists no efforts the
//! typed one (`Effort` `qvWFc` in `Effort and data` `OR6Jq`, `05b` `AZp3I`): under a `border` line, 6 inside and 6
//! apart, a 28-high field on `surface_input` in a `border` stroke with *Type an effort…* and a 12 enter glyph, and
//! under it a hint (11 `text_muted`, 4 in); and the notes (`Data note` `v1AfDF`, `KdDgO`): 6 at the sides and under, 6
//! apart, each a 12 glyph and its 11 `text_secondary` words 8 apart, 7 over and under and 10 at the sides, radius `sm`,
//! on `warning_muted` where it says where data goes, else on `surface_input`.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextInput},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::model::{Notice, NoticeTone};
use crate::{
  theme::{ControlHeight, DISABLED_OPACITY, ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
};

/// A note's padding over and under its words (`KdDgO`): 7.
const NOTE_PADDING: f32 = 7.0;

/// The typed effort: what the field holds and the element that takes the focus.
#[derive(Clone)]
pub(super) struct TypedEffort {
  pub(super) value: Signal<String>,
  pub(super) field: ElementRef,
}

/// The typed effort's field and its hint, named `id`; `active` while there is a model to give an effort to (else the
/// field is inert at 40 %), `current` while the keys are on it. `submit` takes what is typed.
pub(super) fn typed_effort(
  ctx: &mut Ctx,
  id: &str,
  (typed, active, current): (&TypedEffort, bool, bool),
  submit: &Arc<dyn Fn() + Send + Sync>,
) -> Element {
  let (typed_style, placeholder_style) = super::models::input_styles(ctx);
  let words = ctx.t("ui.runsOnPicker.typeEffort");
  let input = TextInput::styled(typed.value.clone(), typed_style)
    .placeholder_style(placeholder_style)
    .caret_color(PaletteColor::TextPrimary)
    .id(format!("{id}-effort-typed"))
    .describe("a11y_name", words.clone())
    .describe("current", current.to_string())
    .placeholder(&words)
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .ref_element(typed.field.clone())
    .single_line();
  let submit = submit.clone();
  let input = if active {
    input.on_key_down(move |event: KeyboardEvent| {
      if event.key == "Enter" && !event.composing && !event.default_prevented() {
        event.prevent_default();
        submit();
      }
    })
  } else {
    input.focusable(false).pointer_events_none()
  };
  let field = Row::new()
    .id(format!("{id}-effort-field"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Sm)
    .child(input)
    .child(icon(Icon::CornerDownLeft, IconSize::Sm, PaletteColor::TextMuted));
  let field = if active { field } else { field.opacity(DISABLED_OPACITY) };
  let hint = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(SpacingSize::Xs))
    .child(
      Text::new(&ctx.t("ui.runsOnPicker.typeEffortHint"))
        .id(format!("{id}-effort-hint"))
        .variant(TypographyStyle::Caption)
        .flex(1.0),
    );
  Column::new()
    .id(format!("{id}-effort"))
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Snug.role())
    .spacing(ExtraSpacing::Snug.role())
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(field)
    .child(hint)
    .into()
}

/// The notes under the efforts, `notes` in their order, named `id`.
pub(super) fn notes(id: &str, notes: &[&Notice]) -> Element {
  let rows: Vec<Element> = notes
    .iter()
    .enumerate()
    .map(|(index, notice)| note(format!("{id}-note-{index}"), notice))
    .collect();
  Column::new()
    .id(format!("{id}-notes"))
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(ExtraSpacing::Snug.role())
        .right(ExtraSpacing::Snug.role())
        .bottom(ExtraSpacing::Snug.role()),
    )
    .spacing(ExtraSpacing::Snug.role())
    .with_children(rows)
    .into()
}

fn note(id: String, notice: &Notice) -> Element {
  let (glyph, background) = match notice.tone {
    NoticeTone::Warning => (PaletteColor::Warning, PaletteColor::WarningMuted),
    NoticeTone::Danger => (PaletteColor::Danger, PaletteColor::SurfaceInput),
    NoticeTone::Neutral => (PaletteColor::TextMuted, PaletteColor::SurfaceInput),
  };
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), NOTE_PADDING))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .background(background)
    .rounded(RadiusSize::Sm)
    .child(icon(notice.icon, IconSize::Sm, glyph))
    .child(
      Text::new(&notice.text)
        .variant(crate::theme::ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}
