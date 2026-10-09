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
}

impl ButtonKind {
  const fn name(self) -> &'static str {
    match self {
      Self::Primary => "primary",
      Self::Secondary => "secondary",
      Self::Ghost => "ghost",
      Self::Danger => "danger",
    }
  }

  /// The color of the icon and the label.
  fn content(self) -> PaletteColor {
    match self {
      Self::Primary => ExtraColor::OnAccent.role(),
      Self::Secondary => PaletteColor::TextPrimary,
      Self::Ghost => PaletteColor::TextSecondary,
      Self::Danger => PaletteColor::Danger,
    }
  }

  fn dressed(self, button: Button) -> Button {
    match self {
      Self::Primary => button.background(PaletteColor::Accent),
      Self::Secondary => button
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong)),
      Self::Ghost => button,
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
      Self::Ghost => button
        .hovered_style(fill(ExtraColor::Hover.role()))
        .active_style(fill(ExtraColor::Pressed.role())),
      Self::Danger => button
        .hovered_style(Style::new().border(Border::inside(BorderSize::Sm, PaletteColor::Danger)))
        .active_style(Style::new().border(Border::inside(BorderSize::Sm, PaletteColor::Danger))),
    }
  }
}

/// What a press does, or why there is none.
#[derive(Clone)]
enum Press {
  Action(Arc<dyn Fn() + Send + Sync>),
  /// The request it made is under way.
  Loading,
  /// It cannot be pressed now, e.g. while the input is incomplete.
  Disabled,
  /// Its feature is not built or not allowed.
  Unavailable,
}

impl Press {
  const fn name(&self) -> &'static str {
    match self {
      Self::Action(_) => "enabled",
      Self::Loading => "loading",
      Self::Disabled => "disabled",
      Self::Unavailable => "unavailable",
    }
  }
}

pub struct ActionButton {
  id: Arc<str>,
  label: Arc<str>,
  icon: Option<Icon>,
  kind: ButtonKind,
  press: Press,
  focus: Focus,
  width: Option<Dimension>,
}

impl ActionButton {
  /// A button of `kind` with `label`; without [`Self::on_click`] it is disabled.
  pub fn new(id: impl Into<Arc<str>>, kind: ButtonKind, label: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      icon: None,
      kind,
      press: Press::Disabled,
      focus: Focus::Pointer,
      width: None,
    }
  }

  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  pub fn on_click(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.press = Press::Action(Arc::new(handler));
    self
  }

  /// The request the button made is under way: the loader turns in place of its icon and it cannot be
  /// pressed again.
  pub fn loading(mut self) -> Self {
    self.press = Press::Loading;
    self
  }

  /// It cannot be pressed now; the caller says why where it matters.
  pub fn disabled(mut self) -> Self {
    self.press = Press::Disabled;
    self
  }

  /// Its feature is not built or not allowed: a lock in place of its icon. Say why beside it with
  /// [`super::unavailable::with_reason`].
  pub fn unavailable(mut self) -> Self {
    self.press = Press::Unavailable;
    self
  }

  /// A button of a form or a card: a Tab stop with the focus ring.
  pub fn in_form(mut self) -> Self {
    self.focus = Focus::Form;
    self
  }

  pub fn width(mut self, width: impl Into<Dimension>) -> Self {
    self.width = Some(width.into());
    self
  }

  /// The icon shown before the label: the loader or the lock replace the button's own.
  fn lead(&self) -> Option<Icon> {
    match self.press {
      Press::Loading => Some(Icon::LoaderCircle),
      Press::Unavailable => Some(Icon::Lock),
      Press::Action(_) | Press::Disabled => self.icon,
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
      .describe("state", button.press.name())
      .describe("label", button.label.clone());
    if let Some(width) = button.width {
      frame = frame.width(width);
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
    match button.press {
      Press::Action(handler) => button
        .focus
        .apply(button.kind.interactive(frame))
        .cursor(CursorIcon::Pointer)
        // A press is the button's own: it does not also press a clickable row around it.
        .on_click(move |event: MouseEvent| {
          event.stop_propagation();
          handler();
        })
        .into(),
      Press::Loading => inert(frame).cursor(CursorIcon::Progress).into(),
      Press::Disabled | Press::Unavailable => inert(frame).opacity(DISABLED_OPACITY).into(),
    }
  }
}
