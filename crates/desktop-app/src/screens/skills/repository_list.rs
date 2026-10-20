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
/// Repository item line TJ0Cb in WMT1W's found sources: one line, 34 high.
const FOUND_ROW_HEIGHT: f32 = 34.0;
/// Both lists show six lines (`cyNh6`), then scroll.
const SHOWN_ROWS: usize = 6;
/// A line's divider (`BorderSize::Sm`): a found list that scrolls hides its cut line's divider under the
/// footer's own.
const DIVIDER: f32 = 1.0;
/// TmfIL/OLgEc/HjGPE: the widest type label is Instruction (55px).
const TYPE_WIDTH: f32 = 55.0;
const SOURCE_WIDTH: f32 = 230.0;
#[derive(Clone)]
struct Props {
  item: ImportItem,
  index: usize,
  last: bool,
  selected: Option<Signal<Chosen>>,
  preview: Option<Signal<usize>>,
  pending: bool,
}
impl PartialEq for Props {
  fn eq(&self, o: &Self) -> bool {
    self.item == o.item
      && self.index == o.index
      && self.last == o.last
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
      .min_height(Dimension::Px(if reviewing { ROW_HEIGHT } else { FOUND_ROW_HEIGHT }))
      .spacing(crate::theme::ExtraSpacing::Loose.role())
      .align_items(if reviewing {
        lurq::layout::Alignment::Start
      } else {
        lurq::layout::Alignment::Center
      })
      .padding(if reviewing {
        Padding::symmetric(crate::theme::ExtraSpacing::Loose.role(), SpacingSize::Sm)
      } else {
        // A one-line source keeps TJ0Cb's 34; a wrapped name keeps room above and below.
        Padding::symmetric(crate::theme::skills::cell_spacing(), SpacingSize::Sm)
      });
    if !reviewing {
      // The last line leaves its divider to the found footer's (`idRGH` over `WYErT`).
      if !p.last {
        row = row.border_bottom(lurq::node::border::Border::inside(BorderSize::Sm, PaletteColor::Border));
      }
      // A source with nothing to import is faded as a whole and has no arrow (`ZjQDR`).
      if p.item.outcome.is_err() {
        row = row.opacity(crate::theme::DISABLED_OPACITY);
      }
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
  let last = items.last().map(|(index, _)| *index);
  let list = VirtualizedList::new(ctx, items);
  let list = if selected.is_some() {
    list.height(Dimension::Px(ROW_HEIGHT * count.min(SHOWN_ROWS) as f32))
  } else if count > SHOWN_ROWS {
    // Found rows grow when a name wraps: the list follows them up to six one-line rows, then scrolls.
    list.max_height(Dimension::Px(FOUND_ROW_HEIGHT * SHOWN_ROWS as f32 - DIVIDER))
  } else {
    list
  };
  list
    .list_key(key)
    .width(Dimension::Pct(100.0))
    .mount_keyed::<ItemRow, _, _, _>(
      |(_, i)| i.source.clone(),
      move |(index, item)| Props {
        item: item.clone(),
        index: *index,
        last: Some(*index) == last,
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
    .width(Dimension::Pct(100.0));
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
          .child(summary.variant(ExtraTypography::Note.role())),
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
    // TJ0Cb/q2J50y: only a source that becomes something points at what it becomes.
    let arrow = p.item.outcome.is_ok().then(|| {
      icon(Icon::ArrowRight, crate::theme::IconSize::Sm, PaletteColor::TextMuted)
        .id(format!("skill-source-arrow-{}", p.index))
    });
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
      .with_children(arrow)
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .child(summary.variant(ExtraTypography::Prose.role())),
      )
  };
  row.into()
}
