//! Buttons (`Button/primary` `yJ2jo`, `Button/secondary` `Dswcx`, `Button/ghost` `cnlv1`, `Button/danger`
//! `O3M8iH`): an optional 14 icon and a 12/500 label, 6 apart, 28 high, 12 at each side, radius 6.
//! While its request is under way a button shows the turning loader in place of its icon and cannot be
//! pressed. A button that cannot be pressed now is faded to half; one whose feature is not built or
//! not allowed shows a lock in place of its icon as well (`Create multiple` `Q9aeZ`), and says why
//! beside it through [`super::unavailable::with_reason`].

use std::sync::Arc;

use lurq::{
  app::{
    events::mouse_event::MouseEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Text},
  core::ElementRef,
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::{Focus, inert},
};
use crate::theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, IconSize};

/// The look of a button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
  /// The one action a screen or card is about: `accent`, its content `on_accent`.
  Primary,
  /// `surface_raised` inside a `border_strong` stroke, its content `text_primary`.
  Secondary,
  /// No fill: its content `text_secondary`, the `hover` fill under the pointer.
  Ghost,
  /// `danger_muted`, its content `danger`.
  Danger,
  /// Destructive text on the ghost button (Secrets inspector, Emd0h).
  DangerGhost,
}

impl ButtonKind {
  const fn name(self) -> &'static str {
    match self {
      Self::Primary => "primary",
      Self::Secondary => "secondary",
      Self::Ghost => "ghost",
      Self::Danger => "danger",
      Self::DangerGhost => "danger-ghost",
    }
  }

  /// The color of the icon and the label.
  fn content(self) -> PaletteColor {
    match self {
      Self::Primary => ExtraColor::OnAccent.role(),
      Self::Secondary => PaletteColor::TextPrimary,
      Self::Ghost => PaletteColor::TextSecondary,
      Self::Danger | Self::DangerGhost => PaletteColor::Danger,
    }
  }

  fn dressed(self, button: Button) -> Button {
    match self {
      Self::Primary => button.background(PaletteColor::Accent),
      Self::Secondary => button
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong)),
      Self::Ghost | Self::DangerGhost => button,
      Self::Danger => button.background(PaletteColor::DangerMuted),
    }
  }

  /// Under the pointer and while pressed. The design draws neither; they follow the shell's `hover` and
  /// `pressed` overlays and the accent's own hover.
  fn interactive(self, button: Button) -> Button {
    let fill = |color: PaletteColor| Style::new().background(color);
    match self {
      Self::Primary => button
        .hovered_style(fill(PaletteColor::AccentHover))
        .active_style(fill(PaletteColor::AccentHover)),
      Self::Secondary => button
        .hovered_style(fill(PaletteColor::Border))
        .active_style(fill(PaletteColor::BorderStrong)),
      Self::Ghost | Self::DangerGhost => button
        .hovered_style(fill(ExtraColor::Hover.role()))
        .active_style(fill(ExtraColor::Pressed.role())),
      Self::Danger => button
        .hovered_style(Style::new().border(Border::inside(BorderSize::Sm, PaletteColor::Danger)))
        .active_style(Style::new().border(Border::inside(BorderSize::Sm, PaletteColor::Danger))),
    }
  }
}

/// Why a button cannot be pressed now. It wins over a handler given in any order.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Hold {
  /// The request it made is under way.
  Loading,
  /// It cannot be pressed now, e.g. while the input is incomplete.
  Disabled,
  /// Its feature is not built or not allowed, for this reason.
  Unavailable(Arc<str>),
}

pub struct ActionButton {
  id: Arc<str>,
  label: Arc<str>,
  icon: Option<Icon>,
  kind: ButtonKind,
  action: Option<Arc<dyn Fn() + Send + Sync>>,
  hold: Option<Hold>,
  focus: Focus,
  /// As wide as its parent, its icon and label centered (`Allow retry` `ae0Pi`).
  fill: bool,
  element: Option<ElementRef>,
}

impl ActionButton {
  /// A button of `kind` with `label`; without [`Self::on_click`] it is disabled.
  pub fn new(id: impl Into<Arc<str>>, kind: ButtonKind, label: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      icon: None,
      kind,
      action: None,
      hold: None,
      focus: Focus::Pointer,
      fill: false,
      element: None,
    }
  }

  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  /// What a press does, unless the button is loading, disabled or unavailable.
  pub fn on_click(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.action = Some(Arc::new(handler));
    self
  }

  /// The request the button made is under way: the loader turns in place of its icon and it cannot be
  /// pressed again.
  pub fn loading(mut self) -> Self {
    self.hold = Some(Hold::Loading);
    self
  }

  /// It cannot be pressed now; the caller says why where it matters.
  pub fn disabled(mut self) -> Self {
    self.hold = Some(Hold::Disabled);
    self
  }

  /// Its feature is not built or not allowed, for `reason`: a lock in place of its icon, and the reason
  /// for tools. Show the reason beside it with [`super::unavailable::with_reason`].
  pub fn unavailable(mut self, reason: impl Into<Arc<str>>) -> Self {
    self.hold = Some(Hold::Unavailable(reason.into()));
    self
  }

  /// As wide as its parent, its icon and label centered.
  pub fn fill_width(mut self) -> Self {
    self.fill = true;
    self
  }

  /// A button of a form or a card: a Tab stop with the focus ring.
  pub fn in_form(mut self) -> Self {
    self.focus = Focus::Form;
    self
  }

  /// `element` refers to the button, e.g. so a screen can move the focus to it.
  pub fn element_ref(mut self, element: ElementRef) -> Self {
    self.element = Some(element);
    self
  }

  const fn state(&self) -> &'static str {
    match (&self.hold, &self.action) {
      (Some(Hold::Loading), _) => "loading",
      (Some(Hold::Unavailable(_)), _) => "unavailable",
      (Some(Hold::Disabled), _) | (None, None) => "disabled",
      (None, Some(_)) => "enabled",
    }
  }

  /// The icon shown before the label: the loader or the lock replace the button's own.
  fn lead(&self) -> Option<Icon> {
    match self.hold {
      Some(Hold::Loading) => Some(Icon::LoaderCircle),
      Some(Hold::Unavailable(_)) => Some(Icon::Lock),
      Some(Hold::Disabled) | None => self.icon,
    }
  }
}

impl From<ActionButton> for Element {
  fn from(button: ActionButton) -> Self {
    let color = button.kind.content();
    let mut frame = Button::empty()
      .id(button.id.clone())
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::horizontal(SpacingSize::Md))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(RadiusSize::Md)
      .describe("kind", button.kind.name())
      .describe("state", button.state())
      .describe("a11y_name", button.label.clone());
    if button.fill {
      frame = frame.width(Dimension::Pct(100.0));
    }
    if let Some(lead) = button.lead() {
      frame = frame.child(icon(lead, IconSize::Md, color.clone()).id(format!("{}-icon", button.id)));
    }
    frame = button.kind.dressed(frame).child(
      Text::new(&button.label)
        .id(format!("{}-label", button.id))
        .variant(TypographyStyle::Button)
        .color(color)
        .nowrap(),
    );
    if let Some(element) = button.element {
      frame = frame.ref_element(element);
    }
    if let Some(Hold::Unavailable(reason)) = &button.hold {
      frame = frame.describe("reason", reason.clone());
    }
    match (button.hold, button.action) {
      (None, Some(handler)) => button
        .focus
        .apply(button.kind.interactive(frame))
        .cursor(CursorIcon::Pointer)
        // A press is the button's own: it does not also press a clickable row around it.
        .on_click(move |event: MouseEvent| {
          event.stop_propagation();
          handler();
        })
        .into(),
      (Some(Hold::Loading), _) => inert(frame).cursor(CursorIcon::Progress).into(),
      _ => inert(frame).opacity(DISABLED_OPACITY).into(),
    }
  }
}
