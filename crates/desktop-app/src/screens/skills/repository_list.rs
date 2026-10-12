//! Repository sources share one row in the empty page and the import review.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text, VirtualizedList},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::agent_files::import::{ImportItem, ItemKind};

use super::repository::{Chosen, item_summary};
use crate::{
  theme::ExtraTypography,
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    toggle::{Toggle, ToggleKind},
  },
};
/// Repository item line TJ0Cb with its explanatory second line.
const ROW_HEIGHT: f32 = 50.0;
/// TmfIL/OLgEc/HjGPE: the widest type label is Instruction (55px).
const TYPE_WIDTH: f32 = 55.0;
const SOURCE_WIDTH: f32 = 230.0;
#[derive(Clone)]
struct Props {
  item: ImportItem,
  index: usize,
  selected: Option<Signal<Chosen>>,
  preview: Option<Signal<usize>>,
  pending: bool,
}
impl PartialEq for Props {
  fn eq(&self, o: &Self) -> bool {
    self.item == o.item
      && self.index == o.index
      && self.pending == o.pending
      && self.selected.is_some() == o.selected.is_some()
  }
}
impl DevtoolsInspectable for Props {}
struct ItemRow;
impl Component for ItemRow {
  type Props = Props;
  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<Props>().clone();
    let reviewing = p.selected.is_some();
    let mut row = Row::new()
      .width(Dimension::Pct(100.0))
      .id(format!("skill-source-row-{}", p.index))
      .min_height(Dimension::Px(if reviewing { ROW_HEIGHT } else { 34.0 }))
      .spacing(crate::theme::ExtraSpacing::Loose.role())
      .align_items(if reviewing {
        lurq::layout::Alignment::Start
      } else {
        lurq::layout::Alignment::Center
      })
      .padding(if reviewing {
        Padding::symmetric(crate::theme::ExtraSpacing::Loose.role(), SpacingSize::Sm)
      } else {
        Padding::horizontal(crate::theme::skills::cell_spacing())
      });
    if !reviewing {
      row = row.border_bottom(lurq::node::border::Border::inside(BorderSize::Sm, PaletteColor::Border));
    }
    row = row.with_children(selection(&p)).child(content(ctx, &p));
    if let Some(preview) = p.preview {
      let chosen = preview.get() == p.index;
      let index = p.index;
      let row = pressable(move || preview.set(index))
        .id(format!("skill-import-preview-{index}"))
        .width(Dimension::Pct(100.0))
        .rounded(lurq::app::theme::RadiusSize::Md)
        .child(row);
      if chosen {
        Element::from(row.background(crate::theme::ExtraColor::Hover.role()))
      } else {
        Element::from(row)
      }
    } else {
      Element::from(row)
    }
  }
}
pub fn listed(
  ctx: &mut Ctx,
  key: &str,
  items: Vec<(usize, ImportItem)>,
  selected: Option<Signal<Chosen>>,
  preview: Option<Signal<usize>>,
  pending: bool,
) -> Element {
  let count = items.len();
  let height = if selected.is_some() { ROW_HEIGHT } else { 34.0 };
  VirtualizedList::new(ctx, items)
    .list_key(key)
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(height * count.min(6) as f32))
    .mount_keyed::<ItemRow, _, _, _>(
      |(_, i)| i.source.clone(),
      move |(index, item)| Props {
        item: item.clone(),
        index: *index,
        selected: selected.clone(),
        preview: preview.clone(),
        pending,
      },
    )
}
fn selection(p: &Props) -> Option<Toggle> {
  if let Some(selected) = p.selected.clone() {
    let source = p.item.source.clone();
    let on = selected.get().0.contains(&source);
    let toggle = Toggle::new(
      format!("skill-import-select-{}", p.index),
      ToggleKind::Checkbox,
      on,
      p.item.source.clone(),
    )
    .hide_label();
    let toggle = if p.item.outcome.is_ok() && !p.pending {
      toggle.on_change(move |on| {
        selected.update(|s| {
          if on {
            s.0.insert(source.clone());
          } else {
            s.0.remove(&source);
          }
        })
      })
    } else {
      toggle
    };
    Some(toggle)
  } else {
    None
  }
}
fn content(ctx: &Ctx, p: &Props) -> Element {
  let source = Text::new(&p.item.source)
    .id(format!("skill-source-{}", p.index))
    .width(Dimension::Pct(100.0))
    .variant(crate::theme::skills::import_path());
  let summary = Text::new(&item_summary(ctx, &p.item))
    .id(format!("skill-source-summary-{}", p.index))
    .width(Dimension::Pct(100.0))
    .variant(ExtraTypography::Note.role());
  let row = Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(crate::theme::ExtraSpacing::Loose.role());
  let row = if p.selected.is_some() {
    row
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(crate::theme::ExtraSpacing::Tight.role())
          .child(source)
          .child(summary),
      )
      .child(
        Text::new(&ctx.t(if p.item.kind == ItemKind::Skill {
          "skills.skillType"
        } else {
          "skills.instructionType"
        }))
        .id(format!("skill-source-type-{}", p.index))
        .width(Dimension::Px(TYPE_WIDTH))
        .variant(ExtraTypography::Note.role()),
      )
  } else {
    row
      .child(icon(
        if p.item.kind == ItemKind::Skill {
          Icon::Folder
        } else {
          Icon::FileText
        },
        crate::theme::IconSize::Base,
        PaletteColor::TextMuted,
      ))
      .child(Column::new().width(Dimension::Px(SOURCE_WIDTH)).child(source))
      .child(icon(
        Icon::ArrowRight,
        crate::theme::IconSize::Sm,
        PaletteColor::TextMuted,
      ))
      .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(summary))
  };
  row.into()
}
