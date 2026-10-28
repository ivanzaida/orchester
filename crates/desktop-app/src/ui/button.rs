//! Buttons (`Button/primary` `yJ2jo`, `Button/secondary` `Dswcx`, `Button/ghost` `cnlv1`, `Button/danger`
//! `O3M8iH`): an optional 14 icon and a 12/500 label, 6 apart, 28 high, 12 at each side, radius 6. A large one is
//! 36 high with a 13/500 label, 16 at each side, 14 when it is not primary (the welcome's `rDzhV`, `fZDtO`).
//! While its request is under way a button shows the turning loader in place of its icon and cannot be
//! pressed. A button that cannot be pressed now is faded to half; one whose feature is not built or
//! not allowed shows a lock in place of its icon as well (`Create multiple` `Q9aeZ`), and says why
//! beside it through [`super::unavailable::with_reason`].

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Button, Row, Text},
  core::ElementRef,
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::{Focus, Press, inert},
};
use crate::{
  navigation::link::Open,
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, IconSize},
};

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
  /// A field-like menu trigger with a trailing chevron (LWah7, Runs after).
  Select,
}

impl ButtonKind {
  const fn name(self) -> &'static str {
    match self {
      Self::Primary => "primary",
      Self::Secondary => "secondary",
      Self::Ghost => "ghost",
      Self::Danger => "danger",
      Self::DangerGhost => "danger-ghost",
      Self::Select => "select",
    }
  }

  /// The color of the icon and the label.
  fn content(self) -> PaletteColor {
    match self {
      Self::Primary => ExtraColor::OnAccent.role(),
      Self::Secondary | Self::Select => PaletteColor::TextPrimary,
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
      Self::Select => button
        .background(PaletteColor::SurfaceInput)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border)),
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
      Self::Secondary | Self::Select => button
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
  /// An unavailable action whose design retains its ordinary glyph (Skills Publish).
  UnavailableWithIcon(Arc<str>),
}

pub struct ActionButton {
  id: Arc<str>,
  label: Arc<str>,
  icon: Option<Icon>,
  kind: ButtonKind,
  action: Option<Press>,
  hold: Option<Hold>,
  focus: Focus,
  /// As wide as its parent, its icon and label centered (`Allow retry` `ae0Pi`).
  fill: bool,
  compact: bool,
  /// 24 high instead of 28, as a Copy inside a command row (`PEnj5`).
  small: bool,
  /// 36 high with a 13/500 label.
  large: bool,
  bounded: bool,
  /// Drawn pressed while what it opened shows.
  held: bool,
  /// What it sets applies something now, e.g. a filter.
  applied: bool,
  /// What it holds is refused: its outline in `danger`.
  invalid: bool,
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
      compact: false,
      small: false,
      large: false,
      bounded: false,
      held: false,
      applied: false,
      invalid: false,
      element: None,
    }
  }

  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  /// The button under another element id.
  pub fn id(mut self, id: impl Into<Arc<str>>) -> Self {
    self.id = id.into();
    self
  }

  /// What a press does, unless the button is loading, disabled or unavailable.
  pub fn on_click(self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.press(Press::run(handler))
  }

  /// A button that navigates: a press opens its place with `open` by a browser's rules (`navigation::link`),
  /// unless the button is loading, disabled or unavailable.
  pub fn opens(self, open: Open) -> Self {
    self.press(Press::Open(open))
  }

  /// What a press does, an action or a place to open, unless the button is loading, disabled or unavailable.
  pub fn press(mut self, press: Press) -> Self {
    self.action = Some(press);
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

  /// Preserve the action glyph while keeping the same unavailable reason and inert behavior.
  /// Skills header p4RJw keeps its upload mark when publication cannot be used.
  pub fn unavailable_with_icon(mut self, reason: impl Into<Arc<str>>) -> Self {
    self.hold = Some(Hold::UnavailableWithIcon(reason.into()));
    self
  }

  /// As wide as its parent, its icon and label centered.
  pub fn fill_width(mut self) -> Self {
    self.fill = true;
    self
  }

  /// A 22-high helper action with a 12 icon and note-size label (ZdkB0).
  pub fn compact(mut self) -> Self {
    self.compact = true;
    self
  }

  /// 24 high, its icon and label as a button's: an action inside a command row (`Command row` `PEnj5`).
  pub fn small(mut self) -> Self {
    self.small = true;
    self
  }

  /// 36 high, its label 13/500: the welcome's actions (`rDzhV`, `fZDtO`).
  pub fn large(mut self) -> Self {
    self.large = true;
    self
  }

  /// Keep a long label within the available width; the full accessible name is retained.
  pub fn bounded_label(mut self) -> Self {
    self.bounded = true;
    self
  }

  /// A button of a form or a card: a Tab stop with the focus ring.
  pub fn in_form(mut self) -> Self {
    self.focus = Focus::Keyboard;
    self
  }

  /// `element` refers to the button, e.g. so a screen can move the focus to it.
  /// Drawn in the `pressed` fill while what it opened shows: the cloud's *Sign out* while its question asks (`U0hgR` in
  /// `p2VeL`).
  pub fn held(mut self, held: bool) -> Self {
    self.held = held;
    self
  }

  /// What it sets applies something now (`Filter · 2` `Dn5JJ` in the Tasks list's redesign): `accent_muted` inside a
  /// `border_strong` stroke, its content `text_primary`.
  pub fn applied(mut self, applied: bool) -> Self {
    self.applied = applied;
    self
  }

  /// What a select-like button holds is refused, e.g. steps that wait for themselves (`ok2Zu` in `g6Ecy`): its
  /// outline in `danger`.
  pub fn invalid(mut self, invalid: bool) -> Self {
    self.invalid = invalid;
    self
  }

  pub fn element_ref(mut self, element: ElementRef) -> Self {
    self.element = Some(element);
    self
  }

  const fn state(&self) -> &'static str {
    match (&self.hold, &self.action) {
      (Some(Hold::Loading), _) => "loading",
      (Some(Hold::Unavailable(_) | Hold::UnavailableWithIcon(_)), _) => "unavailable",
      (Some(Hold::Disabled), _) | (None, None) => "disabled",
      (None, Some(_)) => "enabled",
    }
  }

  /// The icon shown before the label: the loader or the lock replace the button's own.
  fn lead(&self) -> Option<Icon> {
    match self.hold {
      Some(Hold::Loading) => Some(Icon::LoaderCircle),
      Some(Hold::Unavailable(_)) => Some(Icon::Lock),
      Some(Hold::Disabled | Hold::UnavailableWithIcon(_)) | None => self.icon,
    }
  }
}

impl ActionButton {
  /// Its height, and the sizes of its icon and label.
  fn metrics(&self) -> (ControlHeight, IconSize, TypographyStyle) {
    if self.compact {
      (
        ControlHeight::Helper,
        IconSize::Sm,
        crate::theme::ExtraTypography::Note.role(),
      )
    } else if self.small {
      (ControlHeight::Sm, IconSize::Md, TypographyStyle::Button)
    } else if self.large {
      (
        ControlHeight::Lg,
        IconSize::Md,
        crate::theme::ExtraTypography::ButtonLarge.role(),
      )
    } else {
      (ControlHeight::Md, IconSize::Md, TypographyStyle::Button)
    }
  }

  /// The room at each side of its content: 12, a large button's 16, or 14 when it is not primary.
  fn side(&self) -> SpacingSize {
    match (self.large, self.kind) {
      (false, _) => SpacingSize::Md,
      (true, ButtonKind::Primary) => SpacingSize::Lg,
      (true, _) => ExtraSpacing::Inset.role(),
    }
  }

  /// The room the button takes, laid out as it is but as plain words: no role, name, focus or pointer. An unseen
  /// sizer for a slot two labels share (Show and Hide). lurq cannot leave an element out of the accessibility tree,
  /// so the sizer's words are there as plain text.
  pub fn room(self) -> Row {
    let (height, glyph, label) = self.metrics();
    let color = self.kind.content();
    Row::new()
      .height(Dimension::Px(height.value()))
      .padding(Padding::horizontal(self.side()))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .with_children(self.lead().map(|lead| icon(lead, glyph, color.clone())))
      .child(Text::new(&self.label).variant(label).color(color).nowrap())
  }
}

impl From<ActionButton> for Element {
  fn from(button: ActionButton) -> Self {
    let color = if button.applied {
      PaletteColor::TextPrimary
    } else {
      button.kind.content()
    };
    let (height, glyph, label) = button.metrics();
    let mut frame = Button::empty()
      .id(button.id.clone())
      .height(Dimension::Px(height.value()))
      .padding(Padding::horizontal(button.side()))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(RadiusSize::Md)
      .describe("kind", button.kind.name())
      .describe("state", button.state())
      .describe("a11y_name", button.label.clone());
    if button.fill || button.kind == ButtonKind::Select {
      frame = frame.width(Dimension::Pct(100.0));
    }
    if button.bounded {
      frame = frame.max_width(Dimension::Pct(100.0)).min_width(Dimension::Px(0.0));
    }
    if let Some(lead) = button.lead() {
      frame = frame.child(icon(lead, glyph, color.clone()).id(format!("{}-icon", button.id)));
    }
    let label = Text::new(&button.label)
      .id(format!("{}-label", button.id))
      .variant(label)
      .color(color)
      .nowrap();
    let label = if button.bounded {
      label
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
        .text_overflow(lurq::components::TextOverflow::Elipsis)
    } else {
      label
    };
    frame = button.kind.dressed(frame).child(if button.kind == ButtonKind::Select {
      label.flex(1.0).min_width(Dimension::Px(0.0))
    } else {
      label
    });
    if button.kind == ButtonKind::Select {
      frame = frame.child(icon(Icon::ChevronDown, IconSize::Md, PaletteColor::TextMuted));
    }
    if button.held {
      frame = frame.background(ExtraColor::Pressed.role()).describe("held", "true");
    }
    if button.applied {
      frame = frame
        .background(PaletteColor::AccentMuted)
        .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
        .describe("applied", "true");
    }
    if button.invalid {
      frame = frame
        .border(Border::inside(BorderSize::Sm, PaletteColor::Danger))
        .describe("invalid", "true");
    }
    if let Some(element) = button.element {
      frame = frame.ref_element(element);
    }
    if let Some(Hold::Unavailable(reason) | Hold::UnavailableWithIcon(reason)) = &button.hold {
      frame = frame.describe("reason", reason.clone());
    }
    match (button.hold, button.action) {
      (None, Some(press)) => press
        .on(
          button
            .focus
            .apply(button.kind.interactive(frame))
            .cursor(CursorIcon::Pointer),
        )
        .into(),
      (Some(Hold::Loading), _) => inert(frame).cursor(CursorIcon::Progress).into(),
      _ => inert(frame).opacity(DISABLED_OPACITY).into(),
    }
  }
}
