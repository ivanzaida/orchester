//! Import from repository's groups (`Instructions · each becomes a draft project instruction` `A7RnLF`, `Skills ·
//! each becomes a draft skill` `yuQJh` in 02c), on the one import page Instructions and Skills open: per kind, its
//! head over a box with a row per item found — its check, its path and what it becomes or why it cannot be imported.
//! A press on a row opens its preview beside the groups. Choices and the open preview are kept by each item's source,
//! not by its place in the list.
use std::{collections::BTreeSet, sync::Arc};

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::agent_files::import::{ImportItem, ItemKind, Unimportable};

use super::parts;
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, HubSpacing, HubText},
  ui::{
    pressable::pressable,
    skeleton::{self, bone},
    toggle::{Toggle, ToggleKind},
  },
};

/// A row's check while the repository is read: a bone of its size.
const CHECK: f32 = 16.0;

/// Importable items left out of the import, by source: every other importable item is chosen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Unchosen(pub BTreeSet<String>);
impl DevtoolsInspectable for Unchosen {}
impl Unchosen {
  pub(crate) fn chosen(&self, item: &ImportItem) -> bool {
    item.outcome.is_ok() && !self.0.contains(&item.source)
  }

  /// The items the import makes drafts of, in the scan's order.
  pub(crate) fn of(&self, items: &[ImportItem]) -> Vec<ImportItem> {
    items.iter().filter(|item| self.chosen(item)).cloned().collect()
  }
}
/// The item whose preview is open, by source; none opens the first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Viewed(pub Option<String>);
impl DevtoolsInspectable for Viewed {}
impl Viewed {
  pub(crate) fn item<'a>(&self, items: &'a [ImportItem]) -> Option<&'a ImportItem> {
    self
      .0
      .as_ref()
      .and_then(|source| items.iter().find(|item| item.source == *source))
      .or_else(|| items.first())
  }
}

/// What an item becomes, or why it cannot be imported, in the words of the page showing it.
pub(crate) type Detail = fn(&Ctx, &ImportItem) -> Arc<str>;

/// The import review's items: `id` names the page (`<id>-item-<n>`, `<id>-toggle-<n>`, `<id>-path-<n>`,
/// `<id>-detail-<n>`, the groups `<id>-instructions` and `<id>-skills`).
pub(crate) struct Review<'a> {
  pub id: &'a str,
  pub items: &'a [ImportItem],
  pub unchosen: Signal<Unchosen>,
  pub viewed: Signal<Viewed>,
  pub busy: bool,
  pub detail: Detail,
}

impl Review<'_> {
  /// One group per kind found, instructions first.
  pub(crate) fn groups(&self, ctx: &Ctx) -> Vec<Element> {
    let shown = self.viewed.get().item(self.items).map(|item| item.source.clone());
    let chosen = self.unchosen.get();
    [ItemKind::Instruction, ItemKind::Skill]
      .into_iter()
      .filter_map(|kind| {
        let rows = self
          .items
          .iter()
          .enumerate()
          .filter(|(_, item)| item.kind == kind)
          .map(|(index, item)| {
            let current = shown.as_deref() == Some(item.source.as_str());
            self.row(ctx, (index, item), (chosen.chosen(item), current))
          })
          .collect::<Vec<_>>();
        if rows.is_empty() {
          return None;
        }
        let (group, title) = match kind {
          ItemKind::Instruction => ("instructions", "instructions.importInstructionGroup"),
          ItemKind::Skill => ("skills", "instructions.importSkillGroup"),
        };
        let group = format!("{}-{group}", self.id);
        Some(
          parts::group(
            &group,
            parts::head(&group, &ctx.t(title), None, None),
            parts::boxed(&format!("{group}-rows"), rows),
          )
          .into(),
        )
      })
      .collect()
  }

  /// An item's row (`AGENTS.md` `zvcY0`): its check, its path, then what it becomes, in `warning` when it cannot be
  /// imported for want of something the person can add.
  fn row(&self, ctx: &Ctx, (index, item): (usize, &ImportItem), (checked, current): (bool, bool)) -> Element {
    let id = self.id;
    let toggle = Toggle::new(
      format!("{id}-toggle-{index}"),
      ToggleKind::Checkbox,
      checked,
      item.source.clone(),
    )
    .hide_label();
    let toggle = if item.outcome.is_ok() && !self.busy {
      let (unchosen, source) = (self.unchosen.clone(), item.source.clone());
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
    let detail = Text::new(&(self.detail)(ctx, item))
      .id(format!("{id}-detail-{index}"))
      .variant(HubText::Hint.role())
      .width(Dimension::Pct(100.0));
    let detail = if wants_something(item) {
      detail.color(PaletteColor::Warning)
    } else {
      detail
    };
    let (viewed, source) = (self.viewed.clone(), item.source.clone());
    pressable(move || viewed.set(Viewed(Some(source.clone()))))
      .id(format!("{id}-item-{index}"))
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::Start)
      .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
      .spacing(HubSpacing::FieldGap.role())
      .describe("state", if current { "chosen" } else { "enabled" })
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
              .id(format!("{id}-path-{index}"))
              .variant(ExtraTypography::Strong.role())
              .color(if item.outcome.is_ok() {
                PaletteColor::TextPrimary
              } else {
                PaletteColor::TextMuted
              })
              .nowrap(),
          )
          .child(detail),
      )
      .into()
  }
}

/// The import review while the repository is read: the instructions group, its title real, its rows' checks, paths
/// and details bones of the widths `rows` gives, the lower ones fading, in the geometry of [`Review::row`].
pub(crate) fn reading(ctx: &Ctx, id: &str, rows: &[(f32, f32)]) -> Element {
  let rows = rows
    .iter()
    .map(|(path, detail)| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .align_items(Alignment::Start)
        .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
        .spacing(HubSpacing::FieldGap.role())
        .child(bone(CHECK, CHECK))
        .child(
          Column::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .spacing(ExtraSpacing::Tight.role())
            .child(skeleton::line(ctx, ExtraTypography::Strong.role(), *path))
            .child(skeleton::line(ctx, HubText::Hint.role(), *detail)),
        )
    })
    .collect();
  let group = format!("{id}-instructions");
  parts::group(
    &group,
    parts::head(&group, &ctx.t("instructions.importInstructionGroup"), None, None),
    parts::boxed(
      &format!("{group}-rows"),
      skeleton::fading(rows).into_iter().map(Element::from),
    ),
  )
  .into()
}

/// Whether the item cannot be imported for want of something in the repository (`No SKILL.md · cannot import`),
/// rather than because there is nothing in it to import.
const fn wants_something(item: &ImportItem) -> bool {
  matches!(
    item.outcome,
    Err(Unimportable::NoSkillFile | Unimportable::NotText | Unimportable::UnsupportedFiles { .. })
  )
}
