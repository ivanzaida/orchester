//! The Configure section (`Configure head` `hbx5J` in `Zn8dR`): its head, *Configure* with a 12 chevron, folds
//! and unfolds Pipelines and Agents under it. Folded it says how many entries it holds, *Configure · 2* with the
//! chevron pointing right (`DE34j`). The fold is the project's, kept in its feed for the session, so every tab of
//! the project shows the same.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use super::{Items, SECTION_HEAD_HEIGHT, place::Place, section_gap, section_label};
use crate::{
  project::Projects,
  theme::{ExtraColor, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The section's gap, its head, and its entries unless it is folded.
pub(super) fn section(ctx: &mut Ctx, items: &Items<'_>) -> Vec<Element> {
  let folded_state = Projects::of(ctx).feed(items.project).configure_folded;
  let folded = folded_state.get();
  let label = if folded {
    ctx.t_args(
      "shell.nav.configureFolded",
      [("count", Place::CONFIGURE.len().to_string())],
    )
  } else {
    ctx.t("shell.nav.configure")
  };
  let head = pressable(move || folded_state.update(|folded| *folded = !*folded))
    .id("sidebar-configure")
    .describe("a11y_name", label.clone())
    .describe("state", if folded { "folded" } else { "unfolded" })
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SECTION_HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Xs)
    .rounded(RadiusSize::Md)
    .align_items(Alignment::Center)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(section_label(&label))
    .child(icon(
      if folded { Icon::ChevronRight } else { Icon::ChevronDown },
      IconSize::Sm,
      PaletteColor::TextMuted,
    ));
  let mut section = vec![section_gap().into(), head.into()];
  if !folded {
    section.extend(items.group(ctx, &Place::CONFIGURE));
  }
  section
}
