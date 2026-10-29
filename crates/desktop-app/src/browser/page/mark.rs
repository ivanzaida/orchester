//! Where an agent acted (`Agent mark` `XczoY`, `Agent cursor` `D7l36` in `02 · Agent is acting` `UKAkY`): a blue ring
//! around the element its last action reached, as the runner reported its box, and a pointer with the agent's name at
//! the ring's bottom right, while the action is recent. Orchester draws it from the runner's report, not from CDP.

use std::time::Instant;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, HitTestBehavior, dimension::Dimension, padding::Padding},
};
use orchester_ipc::browser::BrowserTabId;

use super::{Scale, activity::agent_name};
use crate::{
  browser::{model::ACTING_FOR, state::Showing},
  theme::{BrowserBorder, BrowserText, ExtraRadius, ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
};

/// How far the ring stands out of the element, and the pointer's overlap of the ring's corner (`XczoY`, `D7l36`).
const RING_OUTSET: f32 = 3.0;
const POINTER_OVERLAP: f32 = 16.0;

/// The ring and the agent's name over the element the agent of `tab` acted on last, while it is recent.
pub fn mark(ctx: &mut Ctx, showing: &Showing, tab: &BrowserTabId, scale: Scale) -> Option<Element> {
  let view = showing.view.as_ref()?;
  let acting = view
    .acting(tab)
    .filter(|acting| Instant::now().saturating_duration_since(acting.at) < ACTING_FOR)?;
  let element = acting.action.element?;
  let name = agent_name(ctx, showing.strip.find(tab)?);
  let left = element.x as f32 * scale.x - RING_OUTSET;
  let top = element.y as f32 * scale.y - RING_OUTSET;
  let width = element.width as f32 * scale.x + 2.0 * RING_OUTSET;
  let height = element.height as f32 * scale.y + 2.0 * RING_OUTSET;
  let ring = Row::new()
    .id("browser-agent-ring")
    .absolute(left, top, Dimension::Px(width), Dimension::Px(height))
    .hit_test(HitTestBehavior::None)
    .rounded(ExtraRadius::Tile.role())
    .border_inside(BrowserBorder::Ring.role(), PaletteColor::Info);
  let pointer = Row::new()
    .id("browser-agent-pointer")
    .absolute(
      left + width - POINTER_OVERLAP,
      top + height - POINTER_OVERLAP / 2.0,
      Dimension::Auto,
      Dimension::Auto,
    )
    .hit_test(HitTestBehavior::None)
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Start)
    .child(icon(Icon::MousePointerClick, IconSize::Lg, PaletteColor::Info))
    .child(
      Row::new()
        .padding(Padding::symmetric(ExtraSpacing::Snug.role(), ExtraSpacing::Tight.role()))
        .rounded(RadiusSize::Sm)
        .background(PaletteColor::Info)
        .child(Text::new(&name).variant(BrowserText::AgentTag.role()).nowrap()),
    );
  Some(
    Row::new()
      .id("browser-agent-mark")
      .absolute(0.0, 0.0, Dimension::Pct(100.0), Dimension::Pct(100.0))
      .hit_test(HitTestBehavior::None)
      .child(ring)
      .child(pointer)
      .into(),
  )
}
