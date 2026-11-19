//! The pinned actions of every panel of the hub (`Actions` `x3uhYJ` in `t4mtQL`, `o8tFHR` in `NDZXC`, `WpN6M` in
//! `SpRvd`, `hDhr8` in `c3jU1R`): the buttons of the fix of the first missing part at its start and,
//! at its end, what the person is told about it.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::{Alignment, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    pressable::Press,
  },
};

/// The bar with `buttons` at its start and `hint` at its end; the kind's panels each say what they put in them.
pub(in crate::screens) fn bar(buttons: Vec<Element>, hint: Option<String>) -> Element {
  bar_saying(buttons, hint.map(|hint| (hint, PaletteColor::MutedForeground)))
}

/// [`bar`] with `hint` in `color`: the form that creates a resource says in `danger` why Create cannot be pressed.
pub(in crate::screens) fn bar_saying(buttons: Vec<Element>, hint: Option<(String, PaletteColor)>) -> Element {
  bar_named("team-hub-actions", buttons, hint)
}

/// [`bar_saying`] under `id`: the bar of a panel in bones is not the panel's.
pub(in crate::screens) fn bar_named(id: &str, buttons: Vec<Element>, hint: Option<(String, PaletteColor)>) -> Element {
  bar_ending(id, buttons, hint, None)
}

/// [`bar_named`] with `end` after the hint, at the bar's end: a memory record's *Delete…* (`BMDBw` in Y2 `oIgSL`).
pub(in crate::screens) fn bar_ending(
  id: &str,
  buttons: Vec<Element>,
  hint: Option<(String, PaletteColor)>,
  end: Option<Element>,
) -> Element {
  let bar = Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    // A panel too narrow for every button takes them on a second line rather than past its edge.
    .min_height(Dimension::Px(HubSpacing::Actions.value()))
    .wrap()
    .padding(Padding::symmetric(HubSpacing::Sections.role(), SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::Canvas)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(buttons);
  // The hint takes what the buttons leave and wraps, so a long one is never cut at the panel's edge.
  match hint {
    Some((hint, color)) => bar.child(
      Text::new(&hint)
        .id("team-hub-actions-hint")
        .variant(HubText::PanelHint.role())
        .color(color)
        .text_align(TextAlign::Right)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    ),
    None => bar.child(Row::new().flex(1.0)),
  }
  .with_children(end)
  .into()
}

/// *Use for all*: how many profiles it sets, and what a press on it does.
pub(in crate::screens) type UseAll = (usize, Press);

/// *Use for all 9*, beside the pinned *Use … for …*: the same account for each of the `count` profiles of the CLI that
/// wait for one on this computer, the open one among them. Its id starts with `prefix`.
pub(in crate::screens) fn use_all(ctx: &Ctx, prefix: &str, count: usize) -> ActionButton {
  ActionButton::new(
    format!("{prefix}-fix-use-all"),
    ButtonKind::Secondary,
    ctx.t_args("team_hub.fix.action.useAll", [("count", count.to_string())]),
  )
  .pinned()
}

/// What the bar says at its end while [`use_all`] stands in it: how many profiles of `cli` wait.
pub(in crate::screens) fn use_all_hint(ctx: &Ctx, count: usize, cli: &str) -> String {
  ctx
    .t_args(
      "team_hub.fix.hint.useAll",
      [("count", &*count.to_string()), ("cli", cli)],
    )
    .to_string()
}
