//! A sub-page's form (`Connection` `H3e3OD` in `10 · Add provider`, `Server` `YiaYW` in `11 · Add MCP server`): its
//! sections as groups, each field a row with its label and what it is for at the start, 240 wide, and its control
//! taking the rest (`Name` `rjWZz`: 11 over and under, 14 at the sides, 16 between); and the page's actions at the
//! end, after why the last cannot be used (`Actions` `cUKmf`).

use lurq::{
  app::theme::{PaletteColor, SpacingSize},
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::Group;
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, ProjectSettingsSpacing, ProjectSettingsText},
  ui::{
    form_section::Label,
    icon::{Icon, icon},
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
      Column::new()
        .width(Dimension::Px(LABEL_WIDTH))
        .flex_shrink(0.0)
        .spacing(ExtraSpacing::Tight.role())
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
        })),
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

/// The page's actions at the end, after why the last cannot be used (`Reason` `fcvLY`), in a line of its own.
pub(crate) fn form_actions(id: &str, actions: Vec<Element>, reason: Option<&str>) -> Element {
  let reason = reason.map(|reason| {
    Row::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted))
      .child(
        Text::new(reason)
          .id(format!("{id}-reason"))
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
