//! A sub-page's form (`Connection` `H3e3OD` in `10 · Add provider`, `Server` `YiaYW` in `11 · Add MCP server`): its
//! sections as groups, each field a row with its label and one short hint line at the start, 240 wide, what the hint
//! leaves out in the label's (i), and its control taking the rest (`Name` `rjWZz`: 11 over and under, 14 at the
//! sides, 16 between); a warning line under a group (`Widens` `tXVIh`, `Sent to` `s4sjZm`); and the page's actions at
//! the end, after why the last cannot be used or what comes next (`Actions` `cUKmf`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::{Group, Tone};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, ProjectSettingsSpacing, ProjectSettingsText},
  ui::{
    form_section::Label,
    icon::{Icon, icon},
    tooltip::Tooltip,
  },
};

/// A field's label column (`Text` `OjB5n`).
const LABEL_WIDTH: f32 = 240.0;

/// A form's section as a group: `title` over its `rows`, `note` at the heading's end.
pub(crate) fn form_group(id: &str, title: &str, note: Option<&str>, rows: Vec<Element>) -> Element {
  let group = Group::new(id, title.to_owned()).rows(rows);
  match note {
    Some(note) => group.end(
      Text::new(note)
        .id(format!("{id}-note"))
        .variant(ExtraTypography::Note.role()),
    ),
    None => group,
  }
  .into()
}

/// A field's row: its label (`<id>-label`) and what it is for (`<id>-description`) at the start, `control` after them.
pub(crate) fn form_row(label: Label, control: impl Into<Element>) -> Element {
  let id = label.id.clone();
  let words = Column::new()
    .child(
      Text::new(&label.label)
        .id(format!("{id}-label"))
        .variant(ExtraTypography::Strong.role()),
    )
    .with_children(label.description.map(|description| {
      Text::new(&description)
        .id(format!("{id}-description"))
        .variant(ProjectSettingsText::Hint.role())
        .width(Dimension::Pct(100.0))
    }));
  field_row(&id, words, control.into())
}

/// A field's row with one short hint line, in its tone, and what the hint leaves out in the label's (i) (`Program`
/// `lzSGF`, `API key` `s7DEcn`): the label `<id>-label`, the hint `<id>-description`, the (i) `<id>-info`.
pub(crate) struct FormField {
  id: Arc<str>,
  label: Arc<str>,
  hint: Option<(Arc<str>, Tone)>,
  info: Option<Arc<str>>,
}

impl FormField {
  pub(crate) fn new(id: &str, label: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      hint: None,
      info: None,
    }
  }

  /// The hint line, `text_muted`.
  pub(crate) fn hint(self, hint: impl Into<Arc<str>>) -> Self {
    self.toned(hint, Tone::Muted)
  }

  /// The hint line in `tone`, e.g. whether the program was found.
  pub(crate) fn toned(mut self, hint: impl Into<Arc<str>>, tone: Tone) -> Self {
    self.hint = Some((hint.into(), tone));
    self
  }

  /// What the hint line leaves out, in the (i) after the label.
  pub(crate) fn info(mut self, info: impl Into<Arc<str>>) -> Self {
    self.info = Some(info.into());
    self
  }

  pub(crate) fn render(self, ctx: &mut Ctx, control: impl Into<Element>) -> Element {
    let id = self.id;
    let info = self.info.map(|info| {
      let glyph = icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted).id(format!("{id}-info"));
      Tooltip::new(format!("{id}-info"), info).wrap(ctx, glyph)
    });
    let words = Column::new()
      .child(
        Row::new()
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .child(
            Text::new(&self.label)
              .id(format!("{id}-label"))
              .variant(ExtraTypography::Strong.role()),
          )
          .with_children(info),
      )
      .with_children(self.hint.map(|(hint, tone)| {
        Text::new(&hint)
          .id(format!("{id}-description"))
          .variant(ProjectSettingsText::Hint.role())
          .color(tone.color())
          .width(Dimension::Pct(100.0))
      }));
    field_row(&id, words, control.into())
  }
}

/// The row of the field `id`: `words` in the label column, `control` taking the rest.
fn field_row(id: &str, words: Column, control: Element) -> Element {
  Row::new()
    .id(format!("{id}-row"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ProjectSettingsSpacing::RowX.role(),
      ProjectSettingsSpacing::RowY.role(),
    ))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Start)
    .child(
      words
        .width(Dimension::Px(LABEL_WIDTH))
        .flex_shrink(0.0)
        .spacing(ExtraSpacing::Tight.role()),
    )
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(control),
    )
    .into()
}

/// A warning line under a group (`Widens` `tXVIh`, `Sent to` `s4sjZm`): a `warning` triangle and what it warns of in
/// 12 `text_secondary`, on `warning_muted`, radius 6, 8 over and under and 10 at its sides; what it leaves out in an
/// (i) at its end (`<id>-info`).
pub(crate) fn warning_line(ctx: &mut Ctx, id: &str, words: &str, info: Option<Arc<str>>) -> Element {
  let info = info.map(|info| {
    let glyph = icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted).id(format!("{id}-info"));
    Tooltip::new(format!("{id}-info"), info).wrap(ctx, glyph)
  });
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .background(PaletteColor::WarningMuted)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::TriangleAlert, IconSize::Base, PaletteColor::Warning))
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(ProjectSettingsText::Hint.role())
        .color(PaletteColor::TextSecondary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(info)
    .into()
}

/// The page's actions at the end, after why the last cannot be used (`Reason` `fcvLY`), in a line of its own.
pub(crate) fn form_actions(id: &str, actions: Vec<Element>, reason: Option<&str>) -> Element {
  form_actions_noted(id, actions, reason.map(|reason| (reason, Tone::Muted)))
}

/// The page's actions after a line in its tone: why the last cannot be used (`text_muted`, an info glyph,
/// `<id>-reason`), or what comes after it (`success`, a check, `uv3UK`, `<id>-next`).
pub(crate) fn form_actions_noted(id: &str, actions: Vec<Element>, note: Option<(&str, Tone)>) -> Element {
  let reason = note.map(|(reason, tone)| {
    let (glyph, part) = match tone {
      Tone::Success => (Icon::CircleCheck, "next"),
      _ => (Icon::Info, "reason"),
    };
    Row::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Sm, tone.color()))
      .child(
        Text::new(reason)
          .id(format!("{id}-{part}"))
          .variant(ProjectSettingsText::Hint.role())
          .flex(1.0),
      )
  });
  let has_reason = reason.is_some();
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Xs))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .with_children(reason)
    .with_children((!has_reason).then(|| Spacer::new().flex(1.0)))
    .child(
      Row::new()
        .id(format!("{id}-buttons"))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(actions),
    )
    .into()
}
