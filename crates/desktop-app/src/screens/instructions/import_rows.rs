//! Grouped, virtualized import list and row zNzKN.
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Text, VirtualizedList},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::agent_files::import::{ImportItem, ItemKind};

use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, instructions},
  ui::{
    pressable::pressable,
    toggle::{Toggle, ToggleKind},
  },
};

#[derive(Clone, PartialEq)]
enum Item {
  Group(ItemKind),
  Entry(usize, ImportItem),
}
#[derive(Clone)]
struct Props {
  item: Item,
  busy: bool,
  selected: Signal<Vec<usize>>,
  preview: Signal<usize>,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.item == other.item && self.busy == other.busy
  }
}
impl DevtoolsInspectable for Props {}
struct ImportRow;
impl Component for ImportRow {
  type Props = Props;
  fn create(_: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    match &props.item {
      Item::Group(kind) => Text::new(&ctx.t(match kind {
        ItemKind::Instruction => "instructions.importInstructionGroup",
        ItemKind::Skill => "instructions.importSkillGroup",
      }))
      .variant(instructions::overline())
      .padding(Padding::new().top(SpacingSize::Md).bottom(SpacingSize::Sm))
      .into(),
      Item::Entry(index, item) => row(ctx, &props, *index, item),
    }
  }
}
fn row(ctx: &Ctx, props: &Props, index: usize, item: &ImportItem) -> Element {
  let checked = props.selected.get().contains(&index);
  let selected = props.selected.clone();
  let toggle = Toggle::new(
    format!("instruction-import-toggle-{index}"),
    ToggleKind::Checkbox,
    checked,
    item.source.clone(),
  )
  .hide_label();
  let toggle = if item.outcome.is_ok() && !props.busy {
    toggle.on_change(move |on| {
      selected.update(|chosen| {
        if on && !chosen.contains(&index) {
          chosen.push(index);
        } else if !on {
          chosen.retain(|i| *i != index);
        }
      })
    })
  } else {
    toggle
  };
  let preview = props.preview.clone();
  let current = preview.get() == index;
  pressable(move || preview.set(index))
    .id(format!("instruction-import-item-{index}"))
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Start)
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .rounded(ExtraRadius::Tile.role())
    .background(if current {
      ExtraColor::Hover.role()
    } else {
      PaletteColor::SurfacePanel
    })
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(toggle)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&item.source)
            .variant(TypographyStyle::Mono)
            .color(if item.outcome.is_ok() {
              PaletteColor::TextPrimary
            } else {
              PaletteColor::TextMuted
            }),
        )
        .child(Text::new(&super::import::item_detail(ctx, item)).variant(TypographyStyle::Description)),
    )
    .child(
      Text::new(&ctx.t(match item.kind {
        ItemKind::Instruction => "instructions.importKindInstruction",
        ItemKind::Skill => "instructions.importKindSkill",
      }))
      .variant(TypographyStyle::Caption),
    )
    .into()
}
pub fn list(
  ctx: &mut Ctx,
  items: &[ImportItem],
  selected: Signal<Vec<usize>>,
  preview: Signal<usize>,
  busy: bool,
) -> Element {
  let mut grouped = Vec::new();
  for kind in [ItemKind::Instruction, ItemKind::Skill] {
    let entries = items
      .iter()
      .enumerate()
      .filter(|(_, item)| item.kind == kind)
      .collect::<Vec<_>>();
    if entries.is_empty() {
      continue;
    }
    grouped.push(Item::Group(kind));
    grouped.extend(
      entries
        .into_iter()
        .map(|(index, item)| Item::Entry(index, item.clone())),
    );
  }
  VirtualizedList::new_shared(ctx, Arc::new(grouped))
    .list_key("instruction-import-rows")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .mount_keyed::<ImportRow, _, _, _>(
      |item| match item {
        Item::Group(kind) => format!("{kind:?}"),
        Item::Entry(_, item) => item.source.clone(),
      },
      move |item| Props {
        item: item.clone(),
        busy,
        selected: selected.clone(),
        preview: preview.clone(),
      },
    )
}
