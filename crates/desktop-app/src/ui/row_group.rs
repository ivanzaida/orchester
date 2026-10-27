//! Grouped rows (`Published` `pnnmh` in `x1lmXb`, `Step` `E3MuZQ` in `IsU5u`): a group's title in 12/600
//! `text_secondary` with its count in 12 `text_muted` and, at the line's end, what acts on the group; then, 8
//! under it, the words that explain it and its rows in one 1 `border` outline, radius 10, each row under a 1 `border`
//! line but the first. A row is 14 in from the outline and 8 over and under its content, 16 between its parts: an
//! optional mark at its start (a radio, a tile), its label in 13/500 `text_primary` with tags after it, a hint 2 under
//! it in 12 `text_muted` (or `warning` / `danger`), and at its end its control, its value in 12.5 `text_secondary` or a
//! chevron when it opens a page. A row a press opens takes the `hover` fill under the pointer and keeps it while it
//! is the one chosen.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::{Press, pressed},
};
use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography, GroupText, IconSize};

/// A titled group of rows.
pub struct RowGroup {
  id: Arc<str>,
  title: Arc<str>,
  count: Option<Arc<str>>,
  end: Option<Element>,
  lead: Option<Arc<str>>,
  rows: Vec<Part>,
  after: Vec<Element>,
}

/// A row in the outline, or what stands between two outlines.
enum Part {
  Row(Element),
  Between(Element),
}

impl RowGroup {
  pub fn new(id: impl Into<Arc<str>>, title: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      title: title.into(),
      count: None,
      end: None,
      lead: None,
      rows: Vec::new(),
      after: Vec::new(),
    }
  }

  /// How many the group holds, or when, beside its title.
  pub fn count(mut self, count: impl Into<Arc<str>>) -> Self {
    self.count = Some(count.into());
    self
  }

  /// What acts on the group, at its title line's end (`Add folder` `g28VPL`).
  pub fn end(mut self, end: impl Into<Element>) -> Self {
    self.end = Some(end.into());
    self
  }

  /// The words that explain the group, between its title and its rows (`go3qz`).
  pub fn lead(mut self, lead: impl Into<Arc<str>>) -> Self {
    self.lead = Some(lead.into());
    self
  }

  pub fn row(mut self, row: impl Into<Element>) -> Self {
    self.rows.push(Part::Row(row.into()));
    self
  }

  pub fn rows(mut self, rows: impl IntoIterator<Item = impl Into<Element>>) -> Self {
    self.rows.extend(rows.into_iter().map(|row| Part::Row(row.into())));
    self
  }

  /// What stands between the rows so far and the next ones, outside the outline, which it splits in two: a notice
  /// about the row above that the rows below follow from (a tool that cannot run, `pcNiB` in `mRLFg`).
  pub fn between(mut self, between: impl Into<Element>) -> Self {
    self.rows.push(Part::Between(between.into()));
    self
  }

  /// What follows the outline: a note, a menu that opens under a row.
  pub fn after(mut self, after: impl Into<Element>) -> Self {
    self.after.push(after.into());
    self
  }
}

/// The group's rows in outlines, split where something stands between them; the first outline is `<id>-rows`.
fn outlines(id: &str, parts: Vec<Part>) -> Vec<Element> {
  let mut shown: Vec<Element> = Vec::new();
  let mut outline: Option<Column> = None;
  let mut count = 0;
  for part in parts {
    match part {
      Part::Row(row) => {
        let line = Column::new().width(Dimension::Pct(100.0)).child(row);
        outline = Some(match outline {
          Some(outline) => outline.child(line.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))),
          None => {
            count += 1;
            let id = if count == 1 {
              format!("{id}-rows")
            } else {
              format!("{id}-rows-{count}")
            };
            Column::new()
              .id(id)
              .width(Dimension::Pct(100.0))
              .rounded(RadiusSize::Lg)
              .border_inside(BorderSize::Sm, PaletteColor::Border)
              .child(line)
          }
        });
      }
      Part::Between(between) => {
        shown.extend(outline.take().map(Element::from));
        shown.push(between);
      }
    }
  }
  shown.extend(outline.map(Element::from));
  shown
}

impl From<RowGroup> for Element {
  fn from(group: RowGroup) -> Self {
    let heading = Row::new()
      .id(format!("{}-heading", group.id))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&group.title)
          .id(format!("{}-title", group.id))
          .variant(GroupText::Title.role()),
      )
      .with_children(group.count.map(|count| {
        Text::new(&count)
          .id(format!("{}-count", group.id))
          .variant(GroupText::Hint.role())
      }))
      .child(Spacer::new().flex(1.0))
      .with_children(group.end);
    let lead = group.lead.map(|lead| {
      Text::new(&lead)
        .id(format!("{}-lead", group.id))
        .variant(GroupText::Hint.role())
        .width(Dimension::Pct(100.0))
    });
    let rows = outlines(&group.id, group.rows);
    Column::new()
      .id(group.id.to_string())
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(heading)
      .with_children(lead)
      .with_children(rows)
      .with_children(group.after)
      .into()
  }
}

/// The color of a row's hint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HintTone {
  #[default]
  Muted,
  Warning,
  Danger,
}

/// One row of a group.
pub struct GroupRow {
  id: Arc<str>,
  label: Arc<str>,
  mono: bool,
  muted: bool,
  leading: Option<Element>,
  tags: Vec<Element>,
  meta: Option<Arc<str>>,
  hint: Option<(Arc<str>, HintTone)>,
  end: Vec<Element>,
  /// Only words at its end: the row is as high as the design's value rows.
  words_end: bool,
  /// Only marks around its label, no control: 10 above and below it and 12 between its parts.
  marked: bool,
  press: Option<Press>,
  chosen: bool,
}

impl GroupRow {
  pub fn new(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      mono: false,
      muted: false,
      leading: None,
      tags: Vec::new(),
      meta: None,
      hint: None,
      end: Vec::new(),
      words_end: false,
      marked: false,
      press: None,
      chosen: false,
    }
  }

  /// The row holds only marks around its label, an icon at its start and maybe one at its end, and no control:
  /// 10 above and below and 12 between its parts (`Triager · Draft v1` `GQxqW`, `ZENDESK_TOKEN` `AEvnI` in `ky5kJ`).
  pub const fn marked(mut self) -> Self {
    self.marked = true;
    self
  }

  /// The label is a path or an identifier, in Geist Mono (`xKL70`).
  pub const fn mono(mut self) -> Self {
    self.mono = true;
    self
  }

  /// The label in `text_muted`: a choice that cannot be made (`y0uwy`).
  pub const fn muted(mut self) -> Self {
    self.muted = true;
    self
  }

  /// A mark at the row's start: a radio, a tile, an icon.
  pub fn leading(mut self, leading: impl Into<Element>) -> Self {
    self.leading = Some(leading.into());
    self
  }

  /// A tag after the label (*Default*, *Changed*).
  pub fn tag(mut self, tag: impl Into<Element>) -> Self {
    self.tags.push(tag.into());
    self
  }

  /// Words after the label in 12 `text_muted` (`zG44z`).
  pub fn meta(mut self, meta: impl Into<Arc<str>>) -> Self {
    self.meta = Some(meta.into());
    self
  }

  /// The hint under the label.
  pub fn hint(self, hint: impl Into<Arc<str>>) -> Self {
    self.toned_hint(hint, HintTone::Muted)
  }

  pub fn toned_hint(mut self, hint: impl Into<Arc<str>>, tone: HintTone) -> Self {
    self.hint = Some((hint.into(), tone));
    self
  }

  /// A control or a mark at the row's end, after the ones added before it.
  pub fn end(mut self, end: impl Into<Element>) -> Self {
    self.end.push(end.into());
    self
  }

  /// The row's value at its end, in 12.5 `text_secondary`.
  pub fn value(self, value: &str) -> Self {
    let text = Text::new(value)
      .id(format!("{}-value", self.id))
      .variant(GroupText::Value.role())
      .nowrap();
    self.words(text)
  }

  /// The row's value at its end, an identifier in Geist Mono.
  pub fn mono_value(self, value: &str) -> Self {
    let text = Text::new(value)
      .id(format!("{}-value", self.id))
      .variant(GroupText::MonoValue.role())
      .nowrap();
    self.words(text)
  }

  fn words(mut self, text: Text) -> Self {
    self.words_end = self.end.is_empty();
    self.end.push(text.into());
    self
  }

  /// A press on the row does `press`.
  pub fn press(mut self, press: Press) -> Self {
    self.press = Some(press);
    self
  }

  /// A press opens a page: the row ends in a chevron (`VXz2i`).
  pub fn opens_page(mut self, press: Press) -> Self {
    self
      .end
      .push(icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted).into());
    self.press = Some(press);
    self
  }

  /// The row is the one chosen: it keeps the `hover` fill.
  pub const fn chosen(mut self, chosen: bool) -> Self {
    self.chosen = chosen;
    self
  }

  /// The label with its tags and meta, and the hint under them.
  fn text(id: &str, (label, mono, muted): (&str, bool, bool), tags: Vec<Element>, meta: Option<&str>) -> Row {
    let label = Text::new(label)
      .id(format!("{id}-label"))
      .variant(if mono {
        GroupText::MonoLabel.role()
      } else {
        ExtraTypography::Strong.role()
      })
      .min_width(Dimension::Px(0.0));
    let label = if muted {
      label.color(PaletteColor::TextMuted)
    } else {
      label
    };
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(label)
      .with_children(tags)
      .with_children(meta.map(|meta| {
        Text::new(meta)
          .id(format!("{id}-meta"))
          .variant(GroupText::Hint.role())
          .nowrap()
      }))
  }
}

fn hint(id: &str, (hint, tone): (&str, HintTone)) -> Text {
  let text = Text::new(hint)
    .id(format!("{id}-hint"))
    .variant(GroupText::Hint.role())
    .width(Dimension::Pct(100.0));
  match tone {
    HintTone::Muted => text,
    HintTone::Warning => text.color(PaletteColor::Warning),
    HintTone::Danger => text.color(PaletteColor::Danger),
  }
}

impl From<GroupRow> for Element {
  fn from(row: GroupRow) -> Self {
    let vertical = if (row.words_end || row.marked) && row.hint.is_none() {
      ExtraSpacing::Loose.role()
    } else {
      SpacingSize::Sm
    };
    let between = if row.marked { SpacingSize::Md } else { SpacingSize::Lg };
    let middle = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Tight.role())
      .child(GroupRow::text(
        &row.id,
        (&row.label, row.mono, row.muted),
        row.tags,
        row.meta.as_deref(),
      ))
      .with_children(row.hint.as_ref().map(|(words, tone)| hint(&row.id, (words, *tone))));
    let padding = Padding::symmetric(ExtraSpacing::Inset.role(), vertical);
    let Some(press) = row.press else {
      return Row::new()
        .id(row.id.to_string())
        .width(Dimension::Pct(100.0))
        .padding(padding)
        .spacing(between)
        .align_items(Alignment::Center)
        .with_children(row.leading)
        .child(middle)
        .with_children(row.end)
        .into();
    };
    let button = pressed(press)
      .id(row.id.to_string())
      .width(Dimension::Pct(100.0))
      .padding(padding)
      .spacing(between)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .describe("chosen", if row.chosen { "true" } else { "false" });
    let button = if row.chosen {
      button.background(ExtraColor::Hover.role())
    } else {
      button
    };
    button
      .with_children(row.leading)
      .child(middle)
      .with_children(row.end)
      .into()
  }
}

/// A tag beside a label: *Default* in a round `surface_raised` chip (`i3mZr`), or *Changed* in `accent_text` in a
/// round `selection` outline (`X4LDh`).
pub fn tag(id: &str, word: &str, changed: bool) -> Row {
  let row = Row::new()
    .id(id.to_owned())
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Tight.role()))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .rounded(crate::theme::ExtraRadius::Full.role());
  if changed {
    row
      .border_inside(BorderSize::Sm, ExtraColor::Selection.role())
      .child(icon(Icon::FilePen, IconSize::Xs, ExtraColor::AccentText.role()))
      .child(Text::new(word).variant(GroupText::Tag.role()).nowrap())
  } else {
    row
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .child(
        Text::new(word)
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextSecondary)
          .nowrap(),
      )
  }
}

/// A note under a group: an `info` glyph and its words in 12 `text_muted` (`m7d9w9`).
pub fn note(id: &str, words: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(GroupText::Hint.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}
