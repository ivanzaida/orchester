//! Grouped, virtualized import list and row zNzKN. Choices and the open preview are kept by each item's
//! source, not by its place in the list.
use std::{collections::BTreeSet, sync::Arc};

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
    tooltip::Truncated,
  },
};

/// Importable items left out of the import, by source: every other importable item is chosen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unchosen(pub BTreeSet<String>);
impl DevtoolsInspectable for Unchosen {}
impl Unchosen {
  pub fn chosen(&self, item: &ImportItem) -> bool {
    item.outcome.is_ok() && !self.0.contains(&item.source)
  }
}
/// The item whose preview is open, by source; none opens the first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Viewed(pub Option<String>);
impl DevtoolsInspectable for Viewed {}
impl Viewed {
  pub fn item<'a>(&self, items: &'a [ImportItem]) -> Option<&'a ImportItem> {
    self
      .0
      .as_ref()
      .and_then(|source| items.iter().find(|item| item.source == *source))
      .or_else(|| items.first())
  }
}
#[derive(Clone, PartialEq)]
enum Item {
  Group(ItemKind),
  Entry(usize, ImportItem),
}
#[derive(Clone)]
struct Props {
  item: Item,
  busy: bool,
  /// The source of the item whose preview is open.
  shown: Option<String>,
  unchosen: Signal<Unchosen>,
  viewed: Signal<Viewed>,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.item == other.item && self.busy == other.busy && self.shown == other.shown
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
fn row(ctx: &mut Ctx, props: &Props, index: usize, item: &ImportItem) -> Element {
  let checked = props.unchosen.get().chosen(item);
  let unchosen = props.unchosen.clone();
  let source = item.source.clone();
  let toggle = Toggle::new(
    format!("instruction-import-toggle-{index}"),
    ToggleKind::Checkbox,
    checked,
    item.source.clone(),
  )
  .hide_label();
  let toggle = if item.outcome.is_ok() && !props.busy {
    toggle.on_change(move |on| {
      unchosen.update(|Unchosen(left_out)| {
        if on {
          left_out.remove(&source);
        } else {
          left_out.insert(source.clone());
        }
      });
    })
  } else {
    toggle
  };
  let viewed = props.viewed.clone();
  let current = props.shown.as_deref() == Some(item.source.as_str());
  let source = item.source.clone();
  pressable(move || viewed.set(Viewed(Some(source.clone()))))
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
          Truncated::new(
            format!("instruction-import-path-{index}"),
            item.source.clone(),
            TypographyStyle::Mono,
          )
          .color(if item.outcome.is_ok() {
            PaletteColor::TextPrimary
          } else {
            PaletteColor::TextMuted
          })
          .render(ctx),
        )
        .child(
          Text::new(&super::import::item_detail(ctx, item))
            .id(format!("instruction-import-detail-{index}"))
            .variant(TypographyStyle::Description),
        ),
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
  unchosen: Signal<Unchosen>,
  viewed: Signal<Viewed>,
  busy: bool,
) -> Element {
  let shown = viewed.get().item(items).map(|item| item.source.clone());
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
        shown: shown.clone(),
        unchosen: unchosen.clone(),
        viewed: viewed.clone(),
      },
    )
}
