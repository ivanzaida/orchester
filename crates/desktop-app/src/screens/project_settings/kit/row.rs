//! A row of a group (`Name` `Pt6ry`, `GitHub repository` `AuIZJ`): its label in 13/500, an (i) whose tooltip holds
//! what the row's one hint line leaves out (`Info` `FQ9Hq`), at most one hint line in 12, `text_muted` or in its tone
//! (`TxE4t` `success`, `mQbP1` `warning`, `Q2wBe` `danger`), and its control at the end, 16 apart; 11 over and under,
//! 14 at its sides. What a state adds under the row (an error, an editor, a question) runs the row's width under it.
//! A row of an item (a connection, a secret, `InVOF`, `JElwG`) is 10 over and under with its parts 12 apart, starts
//! with its tile or glyph, and opens the item when it is pressed, the `hover` fill under the pointer.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use crate::{
  navigation::link::Open,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, ProjectSettingsSpacing, ProjectSettingsText},
  ui::{
    icon::{Icon, icon},
    pressable::opening,
    skeleton,
    tooltip::Tooltip,
  },
};

/// The tone a row's hint line is said in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
  Muted,
  Secondary,
  Success,
  Warning,
  Danger,
}

impl Tone {
  pub(crate) const fn color(self) -> PaletteColor {
    match self {
      Self::Muted => PaletteColor::TextMuted,
      Self::Secondary => PaletteColor::TextSecondary,
      Self::Success => PaletteColor::Success,
      Self::Warning => PaletteColor::Warning,
      Self::Danger => PaletteColor::Danger,
    }
  }
}

pub(crate) struct SettingRow {
  id: Arc<str>,
  label: Arc<str>,
  label_style: TypographyStyle,
  label_color: Option<PaletteColor>,
  cut_label: bool,
  info: Option<Arc<str>>,
  hint: Option<(Arc<str>, Tone)>,
  hint_style: TypographyStyle,
  /// Lines under the hint, in the words' column.
  lines: Vec<Element>,
  lead: Option<Element>,
  controls: Vec<Element>,
  under: Vec<Element>,
  open: Option<Open>,
  item: bool,
  /// The label's and the hint's bones, while they are read.
  bones: (Option<f32>, Option<f32>),
}

impl SettingRow {
  /// The row `id` (its frame `<id>-row`, its label `<id>-label`, its hint `<id>-hint`) labelled `label`.
  pub(crate) fn new(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      label_style: ExtraTypography::Strong.role(),
      label_color: None,
      cut_label: false,
      info: None,
      hint: None,
      hint_style: ProjectSettingsText::Hint.role(),
      lines: Vec::new(),
      lead: None,
      controls: Vec::new(),
      under: Vec::new(),
      open: None,
      item: false,
      bones: (None, None),
    }
  }

  /// The hint line is a value as written (a variable's): in Geist Mono 12.
  pub(crate) fn mono_hint(mut self) -> Self {
    self.hint_style = ProjectSettingsText::MonoHint.role();
    self
  }

  /// Lines of their own under the hint, in the words' column beside the control, the words then 4 apart (`Moved`
  /// `HcizH` in 04b `NDTCw`).
  pub(crate) fn lines(mut self, lines: impl IntoIterator<Item = Element>) -> Self {
    self.lines.extend(lines);
    self
  }

  /// A row of an item: 10 over and under, its parts 12 apart.
  pub(crate) fn item(mut self) -> Self {
    self.item = true;
    self
  }

  /// The label is an identifier (a secret's name): in Geist Mono 12.5/500.
  pub(crate) fn mono_label(mut self) -> Self {
    self.label_style = ProjectSettingsText::MonoLabel.role();
    self
  }

  /// The label in 12.5/500, as a source's path is (`wD1ye`).
  pub(crate) fn small_label(mut self) -> Self {
    self.label_style = ExtraTypography::OptionTitle.role();
    self
  }

  /// The label in `color`, e.g. a quiet empty row's title.
  pub(crate) fn label_color(mut self, color: PaletteColor) -> Self {
    self.label_color = Some(color);
    self
  }

  /// The label stays on one line and ends in an ellipsis where the row runs out, e.g. a list of names.
  pub(crate) fn cut_label(mut self) -> Self {
    self.cut_label = true;
    self
  }

  /// What the hint line leaves out, in the (i) after the label.
  pub(crate) fn info(mut self, info: impl Into<Arc<str>>) -> Self {
    self.info = Some(info.into());
    self
  }

  /// The hint line, `text_muted`.
  pub(crate) fn hint(self, hint: impl Into<Arc<str>>) -> Self {
    self.toned(hint, Tone::Muted)
  }

  /// The hint line in `tone`.
  pub(crate) fn toned(mut self, hint: impl Into<Arc<str>>, tone: Tone) -> Self {
    self.hint = Some((hint.into(), tone));
    self
  }

  /// The label is not read yet: its bone, `width` long, in its place.
  pub(crate) fn label_reading(mut self, width: f32) -> Self {
    self.bones.0 = Some(width);
    self
  }

  /// The hint line is not read yet: its bone, `width` long, in its place.
  pub(crate) fn hint_reading(mut self, width: f32) -> Self {
    self.bones.1 = Some(width);
    self
  }

  /// What starts the row: an item's tile or glyph.
  pub(crate) fn lead(mut self, lead: impl Into<Element>) -> Self {
    self.lead = Some(lead.into());
    self
  }

  /// A control at the row's end; several sit 8 apart.
  pub(crate) fn control(mut self, control: impl Into<Element>) -> Self {
    self.controls.push(control.into());
    self
  }

  pub(crate) fn controls(mut self, controls: impl IntoIterator<Item = Element>) -> Self {
    self.controls.extend(controls);
    self
  }

  /// What a state adds under the row, the row's width.
  pub(crate) fn under(mut self, under: impl Into<Element>) -> Self {
    self.under.push(under.into());
    self
  }

  pub(crate) fn under_all(mut self, under: impl IntoIterator<Item = Element>) -> Self {
    self.under.extend(under);
    self
  }

  /// A press on the row opens its item with `open` by a browser's rules (`navigation::link`).
  pub(crate) fn opens(mut self, open: Open) -> Self {
    self.open = Some(open);
    self
  }

  /// The label, or its bone while it is read.
  fn label(&self, ctx: &mut Ctx) -> Element {
    if let Some(width) = self.bones.0 {
      return skeleton::line(ctx, self.label_style, width).into();
    }
    let label = Text::new(&self.label)
      .id(format!("{}-label", self.id))
      .variant(self.label_style);
    let label = match &self.label_color {
      Some(color) => label.color(color.clone()),
      None => label,
    };
    // A label that is a list of names stays on its line and ends in an ellipsis where the row runs out.
    if self.cut_label {
      label
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
        .into()
    } else {
      label.into()
    }
  }

  /// The hint under the label, or its bone while it is read.
  fn hint_line(&self, ctx: &mut Ctx) -> Option<Element> {
    if let Some((hint, tone)) = &self.hint {
      return Some(
        Text::new(hint)
          .id(format!("{}-hint", self.id))
          .variant(self.hint_style)
          .color(tone.color())
          .width(Dimension::Pct(100.0))
          .into(),
      );
    }
    self
      .bones
      .1
      .map(|width| skeleton::line(ctx, self.hint_style, width).into())
  }

  pub(crate) fn render(self, ctx: &mut Ctx) -> Element {
    let (label, hint) = (self.label(ctx), self.hint_line(ctx));
    let id = self.id;
    let info = self.info.map(|info| {
      let glyph = icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted).id(format!("{id}-info"));
      Tooltip::new(format!("{id}-info"), info).wrap(ctx, glyph)
    });
    let heading = Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(label)
      .with_children(info);
    let heading = if self.cut_label {
      heading.width(Dimension::Pct(100.0)).min_width(Dimension::Px(0.0))
    } else {
      heading
    };
    let words = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(if self.lines.is_empty() {
        ExtraSpacing::Tight.role()
      } else {
        SpacingSize::Xs
      })
      .child(heading)
      .with_children(hint)
      .with_children(self.lines);
    let line = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(if self.item { SpacingSize::Md } else { SpacingSize::Lg })
      .align_items(Alignment::Center)
      .with_children(self.lead)
      .child(words)
      .with_children((!self.controls.is_empty()).then(|| {
        Row::new()
          .flex_shrink(0.0)
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .with_children(self.controls)
      }));
    let over = if self.item {
      ExtraSpacing::Loose.role()
    } else {
      ProjectSettingsSpacing::RowY.role()
    };
    let frame = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ProjectSettingsSpacing::RowX.role(), over))
      .spacing(SpacingSize::Sm)
      .child(line)
      .with_children(self.under);
    match self.open {
      Some(open) => opening(open)
        .id(format!("{id}-row"))
        .width(Dimension::Pct(100.0))
        .hovered_style(Style::new().background(ExtraColor::Hover.role()))
        .child(frame)
        .into(),
      None => frame.id(format!("{id}-row")).into(),
    }
  }
}

/// A read-only value at a row's end (`Value` `RWwEb`), cut with an ellipsis past a control's width so a long path or
/// address leaves the row its label and hint.
pub(crate) fn value(id: &str, words: &str) -> Text {
  Text::new(words)
    .id(id.to_owned())
    .variant(ProjectSettingsText::Value.role())
    .nowrap()
    .max_width(Dimension::Px(crate::screens::project_settings::page::CONTROL_WIDTH))
    .text_overflow(TextOverflow::Elipsis)
}

/// A read-only value that is an identifier, a path or an address, in Geist Mono (`boX03`, `W2lRBk` in `01b`).
pub(crate) fn mono_value(id: &str, words: &str) -> Text {
  value(id, words).variant(ProjectSettingsText::MonoValue.role())
}
