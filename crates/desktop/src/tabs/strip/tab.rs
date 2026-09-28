//! One title bar tab (`Window / Tab`, `nAKyb`): 28 high, the project's avatar (or the folders icon
//! of a tab without a project), the label ending with an ellipsis, and the close button. Active
//! tabs are `$surface` and join the screen below; inactive ones have no fill and a muted label, and
//! keep their close slot invisible so the label does not move on hover. The attention dot sits on
//! the avatar's top-right corner with a ring in the tab's own background. Tabs are pointer-only:
//! never focused, no ring, no Tab stop.

use std::sync::Arc;

use lurq::{
  app::{
    events::{DragEvent, MouseButton, MouseEvent},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Stack, Text, TextOverflow},
  core::ElementRef,
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};

use super::super::{
  label::{TabLabel, TabMark},
  model::TabId,
};
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraShadow, ExtraSpacing, ExtraTypography},
  ui::icon::{Icon, IconSize, icon},
};

/// Tab height (`nAKyb`), its avatar and close button (`b9QQb`, `OvWLh`), and where the attention dot
/// sits on the avatar (`RbADO`: 8px, at x 10, y −2).
pub const TAB_HEIGHT: f32 = 28.0;
const AVATAR_SIZE: f32 = 16.0;
const CLOSE_SIZE: f32 = 16.0;
const DOT_SIZE: f32 = 8.0;
const DOT_X: f32 = 10.0;
const DOT_Y: f32 = -2.0;

/// How a tab looks now.
#[derive(Clone)]
pub struct TabView {
  pub id: TabId,
  pub label: TabLabel,
  pub active: bool,
  pub hovered: bool,
  pub close_hovered: bool,
  /// Lifted while dragged to a new place.
  pub dragged: bool,
  /// Whether the tab has a close slot: not a sole All projects tab, not an inactive tab too
  /// narrow for it.
  pub close_slot: bool,
  /// Whether the tab's project wants attention: the dot shows.
  pub attention: bool,
  pub width: f32,
}

impl TabView {
  /// The tab's fill, which the attention dot's ring takes too.
  fn fill(&self) -> PaletteColor {
    if self.dragged {
      PaletteColor::SurfaceRaised
    } else if self.active {
      ExtraColor::Surface.role()
    } else if self.hovered {
      PaletteColor::SurfacePanel
    } else {
      PaletteColor::SurfaceBase
    }
  }

  fn emphasized(&self) -> bool {
    self.active || self.hovered || self.dragged
  }
}

/// What a tab does with the pointer; none while a dialog is open.
pub struct TabHandlers {
  pub activate: Arc<dyn Fn() + Send + Sync>,
  pub close: Arc<dyn Fn() + Send + Sync>,
  pub hover: Arc<dyn Fn(bool) + Send + Sync>,
  pub hover_close: Arc<dyn Fn(bool) + Send + Sync>,
  pub drag_move: Arc<dyn Fn(f32) + Send + Sync>,
  /// The drag ended with the pointer `y` from the title bar's top.
  pub drag_end: Arc<dyn Fn(f32) + Send + Sync>,
}

pub fn tab(view: &TabView, anchor: Option<&ElementRef>, handlers: Option<&TabHandlers>) -> Element {
  let text_color = if view.emphasized() {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  let mut row = Row::new()
    .id(format!("window-tab-{}", view.id))
    .describe("tab", view.label.full.clone())
    .describe("active", if view.active { "true" } else { "false" })
    .width(Dimension::Px(view.width))
    .height(Dimension::Px(TAB_HEIGHT))
    .padding_left(ExtraSpacing::Inline.role())
    .padding_right(ExtraSpacing::Tight.role())
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(view.fill())
    .child(mark(view))
    .child(
      Text::new(&view.label.text)
        .variant(TypographyStyle::FieldLabel)
        .color(text_color)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0),
    )
    .with_children(view.close_slot.then(|| close_button(view, handlers)));
  if view.active {
    row = row.class("window-tab-active");
  }
  row = if view.dragged {
    row.rounded(RadiusSize::Md).box_shadow(ExtraShadow::Popover.role())
  } else {
    row
      .corner_radius_top_left(RadiusSize::Md)
      .corner_radius_top_right(RadiusSize::Md)
  };
  if let Some(anchor) = anchor {
    row = row.ref_element(anchor.clone());
  }
  match handlers {
    Some(handlers) => with_handlers(row, handlers).into(),
    None => row.into(),
  }
}

fn with_handlers(row: Row, handlers: &TabHandlers) -> Row {
  let (activate, close, enter, leave) = (
    handlers.activate.clone(),
    handlers.close.clone(),
    handlers.hover.clone(),
    handlers.hover.clone(),
  );
  let (moved, ended) = (handlers.drag_move.clone(), handlers.drag_end.clone());
  row
    .cursor(CursorIcon::Pointer)
    // A press on a tab is not a press on the title bar's drag area. Its default stays, so the
    // press can start dragging the tab.
    .on_mouse_down(|event: MouseEvent| event.stop_propagation())
    .on_click(move |_: MouseEvent| activate())
    .on_mouse_click(MouseButton::Middle, move |_: MouseEvent| close())
    .on_mouse_enter(move || enter(true))
    .on_mouse_leave(move || leave(false))
    .start_drag_buttons(lurq::app::events::MouseButtonMask::LEFT)
    .on_drag_move(move |event: DragEvent| moved(event.total_delta_x))
    .on_drag_end(move |event: DragEvent| ended(event.y))
}

/// The project's avatar with its initial, or the folders icon of a tab without a project.
fn mark(view: &TabView) -> Element {
  let TabMark::Project { initial } = &view.label.mark else {
    return icon(Icon::Folders, IconSize::Dismiss, PaletteColor::TextSecondary).into();
  };
  let dot_id = format!("window-tab-attention-{}", view.id);
  avatar(initial, view.attention.then_some((dot_id.as_str(), view.fill())))
}

/// A project's avatar (`b9QQb`): its initial on a 16px tile and, when the project wants attention,
/// the dot on the tile's top-right corner (`RbADO`: 8px `$text`, at x 10, y −2) with a 2px ring in
/// `ring`, the background around it.
pub(super) fn avatar(initial: &str, attention: Option<(&str, PaletteColor)>) -> Element {
  let tile = Row::new()
    .size(Dimension::Px(AVATAR_SIZE), Dimension::Px(AVATAR_SIZE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::AccentMuted)
    .rounded(ExtraRadius::TabControl.role())
    .child(Text::new(initial).variant(ExtraTypography::TabInitial.role()));
  let Some((id, ring)) = attention else {
    return tile.into();
  };
  let dot = Row::new()
    .id(id.to_owned())
    .size(Dimension::Px(DOT_SIZE), Dimension::Px(DOT_SIZE))
    .background(PaletteColor::TextPrimary)
    .border(Border::inside(BorderSize::Md, ring))
    .rounded(ExtraRadius::Pill.role());
  // The dot reaches past the tile's corner.
  Stack::new()
    .size(Dimension::Px(AVATAR_SIZE), Dimension::Px(AVATAR_SIZE))
    .overflow_visible()
    .child(tile)
    .child(Column::new().absolute_position(DOT_X, DOT_Y).child(dot))
    .into()
}

/// The close button (`OvWLh`): invisible on an inactive tab until the pointer is over the tab,
/// `$hover` with a `$text` icon under the pointer.
fn close_button(view: &TabView, handlers: Option<&TabHandlers>) -> Element {
  let shown = view.active || view.hovered;
  let icon_color = if view.close_hovered {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  let mut button = Row::new()
    .id(format!("window-tab-close-{}", view.id))
    .size(Dimension::Px(CLOSE_SIZE), Dimension::Px(CLOSE_SIZE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::TabControl.role())
    .opacity(if shown { 1.0 } else { 0.0 })
    .child(icon(Icon::X, IconSize::Small, icon_color));
  if view.close_hovered {
    button = button.background(PaletteColor::AccentMuted);
  }
  let Some(handlers) = handlers else {
    return button.into();
  };
  let (close, enter, leave) = (
    handlers.close.clone(),
    handlers.hover_close.clone(),
    handlers.hover_close.clone(),
  );
  button
    .cursor(CursorIcon::Pointer)
    // Neither the tab's drag nor the window's starts on the close button.
    .on_mouse_down(|event: MouseEvent| {
      event.prevent_default();
      event.stop_propagation();
    })
    .on_click(move |event: MouseEvent| {
      event.stop_propagation();
      close();
    })
    .on_mouse_enter(move || enter(true))
    .on_mouse_leave(move || leave(false))
    .into()
}
