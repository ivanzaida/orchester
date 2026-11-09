//! Buttons (`Button/primary` `yJ2jo`, `Button/secondary` `Dswcx`, `Button/ghost` `cnlv1`, `Button/danger`
//! `O3M8iH`): an optional 14 icon and a 12/500 label, 6 apart, 28 high, 12 at each side, radius 6. A large one is
//! 36 high with a 13/500 label, 16 at each side, 14 when it is not primary (the welcome's `rDzhV`, `fZDtO`).
//! While its request is under way a button shows the turning loader in place of its icon and cannot be
//! pressed. A button that cannot be pressed now is faded to half; one whose feature is not built or
//! not allowed shows a lock in place of its icon as well (`Create multiple` `Q9aeZ`), and says why
//! beside it through [`super::unavailable::with_reason`].
//!
//! An answer of a decision card with a consequence is as wide as its parent, its label and the consequence under it at
//! its start, 8 over and under and 12 at its sides (`Option · Skip the exports` `wWuSM`). No button draws the keys that
//! press it in a cap; the caller handles them.

mod kind;

use std::sync::Arc;

pub use kind::ButtonKind;
use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Button, Column, Row, Text},
  core::ElementRef,
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::{Focus, Press, inert},
};
use crate::{
  navigation::link::Open,
  theme::{
    CONSEQUENCE_ON_ACCENT_OPACITY, ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize,
  },
};

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
  /// Drawn as it is, but it no longer takes a press: an answer of a card that was answered.
  Shown,
}

pub struct ActionButton {
  id: Arc<str>,
  label: Arc<str>,
  icon: Option<Icon>,
  after: Option<Icon>,
  kind: ButtonKind,
  action: Option<Press>,
  hold: Option<Hold>,
  focus: Focus,
  /// As wide as its parent, its icon and label centered (`Allow retry` `ae0Pi`).
  fill: bool,
  compact: bool,
  /// 10 at each side instead of 12 (`SHKod`).
  tight: bool,
  /// On a group's heading line (`Hfjkt`).
  in_heading: bool,
  /// 24 high instead of 28, as a Copy inside a command row (`PEnj5`).
  small: bool,
  /// 26 high instead of 28, as an action of a review thread (`SYiTG`).
  snug: bool,
  /// 30 high instead of 28, as the pinned action of a hub panel (`Edit` `psiyx` in `hDhr8`).
  pinned: bool,
  /// 36 high with a 13/500 label.
  large: bool,
  /// Large and not primary, with a primary's room at each side.
  roomy: bool,
  bounded: bool,
  /// Drawn pressed while what it opened shows.
  held: bool,
  /// What it sets applies something now, e.g. a filter.
  applied: bool,
  /// What it holds is refused: its outline in `danger`.
  invalid: bool,
  /// As wide as its parent, its words at its start, 8 over and under them.
  stacked: bool,
  /// What a press leads to, under its label, in a stacked button.
  consequence: Option<Arc<str>>,
  element: Option<ElementRef>,
  on_focus: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl ActionButton {
  /// A button of `kind` with `label`; without [`Self::on_click`] it is disabled.
  pub fn new(id: impl Into<Arc<str>>, kind: ButtonKind, label: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      icon: None,
      after: None,
      kind,
      action: None,
      hold: None,
      focus: Focus::Pointer,
      fill: false,
      compact: false,
      tight: false,
      in_heading: false,
      small: false,
      snug: false,
      pinned: false,
      large: false,
      roomy: false,
      bounded: false,
      held: false,
      applied: false,
      invalid: false,
      stacked: false,
      consequence: None,
      element: None,
      on_focus: None,
    }
  }

  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  /// `icon` after the label, where a press goes on to a next step (`Sign in →` in `j7SbPk`); gone while the button is
  /// held, when the loader says it is under way.
  pub fn after(mut self, icon: Icon) -> Self {
    self.after = Some(icon);
    self
  }

  /// Updates supporting copy when the keyboard focuses this answer (06b I8MyfC).
  pub fn on_focus(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.on_focus = Some(Arc::new(handler));
    self
  }

  /// As wide as its parent, its icon and label at its start, 8 over and under them: an answer of an answered card
  /// that had a consequence (`vl5ms` in `hm5wE`).
  pub fn stacked(mut self) -> Self {
    self.stacked = true;
    self
  }

  /// What a press leads to, under the label: the button is [`Self::stacked`] (`wWuSM`).
  pub fn consequence(mut self, words: impl Into<Arc<str>>) -> Self {
    self.stacked = true;
    self.consequence = Some(words.into());
    self
  }

  /// Drawn as it is, but it no longer takes a press: an answer of a card that was answered (`hm5wE`).
  pub fn shown(mut self) -> Self {
    self.hold = Some(Hold::Shown);
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

  /// 10 at each side instead of 12: a compact action of the status bar (`Button/Size/Compact` `SHKod` in
  /// `Status bar · redesign`).
  pub fn tight(mut self) -> Self {
    self.tight = true;
    self
  }

  /// As high as a heading's line, with a 12 icon: an action on a group's heading line, so a heading with one starts and
  /// ends where any other does (`Add provider` in `Head` `Hfjkt`).
  pub fn in_heading(mut self) -> Self {
    self.in_heading = true;
    self
  }

  /// 24 high, its icon and label as a button's: an action inside a command row (`Command row` `PEnj5`).
  pub fn small(mut self) -> Self {
    self.small = true;
    self
  }

  /// 26 high, its icon and label as a button's: an action in a review thread's head or the comment box (`Ask again`
  /// `SYiTG`, `Add comment` `Z0RVD` in `Review · redesign`).
  pub fn snug(mut self) -> Self {
    self.snug = true;
    self
  }

  /// 30 high, its icon and label as a button's: the action pinned at the bottom of a hub panel (`Edit` `psiyx` in
  /// `hDhr8`).
  pub fn pinned(mut self) -> Self {
    self.pinned = true;
    self
  }

  /// 36 high, its label 13/500: the welcome's actions (`rDzhV`, `fZDtO`).
  pub fn large(mut self) -> Self {
    self.large = true;
    self
  }

  /// A large button that is not primary with 16 at each side, as the start page's steps draw it (`Back` `eMkuS`,
  /// `Choose…` `a0csJ`).
  pub fn roomy(mut self) -> Self {
    self.roomy = true;
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
      (Some(Hold::Shown), _) => "shown",
      (Some(Hold::Disabled), _) | (None, None) => "disabled",
      (None, Some(_)) => "enabled",
    }
  }

  /// The icon shown before the label: the loader or the lock replace the button's own.
  fn lead(&self) -> Option<Icon> {
    match self.hold {
      Some(Hold::Loading) => Some(Icon::LoaderCircle),
      Some(Hold::Unavailable(_)) => Some(Icon::Lock),
      Some(Hold::Disabled | Hold::UnavailableWithIcon(_) | Hold::Shown) | None => self.icon,
    }
  }
}

impl ActionButton {
  /// Its height, and the sizes of its icon and label.
  fn metrics(&self) -> (ControlHeight, IconSize, TypographyStyle) {
    if self.in_heading {
      (ControlHeight::Heading, IconSize::Sm, TypographyStyle::Button)
    } else if self.compact {
      (
        ControlHeight::Helper,
        IconSize::Sm,
        crate::theme::ExtraTypography::Note.role(),
      )
    } else if self.small {
      (ControlHeight::Sm, IconSize::Md, TypographyStyle::Button)
    } else if self.snug {
      (ControlHeight::Snug, IconSize::Md, TypographyStyle::Button)
    } else if self.pinned {
      (ControlHeight::Row, IconSize::Md, TypographyStyle::Button)
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

  /// The room at each side of its content: 12, a tight one's 10, a large button's 16, or 14 when it is not primary and
  /// not [`Self::roomy`].
  fn side(&self) -> SpacingSize {
    match (self.large, self.kind) {
      (false, _) if self.tight => ExtraSpacing::Loose.role(),
      (false, _) => SpacingSize::Md,
      (true, ButtonKind::Primary) => SpacingSize::Lg,
      (true, _) if self.roomy => SpacingSize::Lg,
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

/// `button` as wide as the widest of `sizes`, each laid out as a button with its own words: a slot whose label changes
/// with what is open keeps one width, so nothing beside it moves (the Agents hub's New of each kind). The sizers take no
/// height and show nothing.
pub fn as_wide_as_widest(id: &str, button: ActionButton, sizes: impl IntoIterator<Item = ActionButton>) -> Column {
  let sizers = sizes.into_iter().map(|size| {
    Row::new()
      .height(Dimension::Px(0.0))
      .clip()
      .opacity(0.0)
      .pointer_events_none()
      .child(size.room())
  });
  Column::new()
    .id(id.to_owned())
    .align_items(Alignment::Stretch)
    .with_children(sizers)
    .child(button.fill_width())
}

impl ActionButton {
  /// The frame without its content: 28 high with its content centered, or stacked as wide as its parent, 8 over
  /// and under and 12 at the sides, its content at the start.
  fn frame(&self) -> Button {
    let (height, ..) = self.metrics();
    let frame = Button::empty()
      .id(self.id.clone())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .describe("kind", self.kind.name())
      .describe("state", self.state())
      .describe("a11y_name", self.label.clone());
    let frame = if self.stacked {
      frame
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
        .spacing(SpacingSize::Md)
        .justify(Justify::Start)
    } else {
      frame
        .height(Dimension::Px(height.value()))
        .padding(Padding::horizontal(self.side()))
        .spacing(ExtraSpacing::Snug.role())
        .justify(Justify::Center)
    };
    let frame = if self.fill || self.kind == ButtonKind::Select {
      frame.width(Dimension::Pct(100.0))
    } else {
      frame
    };
    if self.bounded {
      return frame.max_width(Dimension::Pct(100.0)).min_width(Dimension::Px(0.0));
    }
    frame
  }

  /// The label in `color`; stacked, the label and the consequence under it.
  fn slot(&self, color: PaletteColor) -> Element {
    let (_, _, role) = self.metrics();
    let label = Text::new(&self.label)
      .id(format!("{}-label", self.id))
      .color(color)
      .nowrap();
    if self.stacked {
      let words = self.consequence.as_ref().map(|consequence| {
        let words = Text::new(consequence)
          .id(format!("{}-consequence", self.id))
          .variant(ExtraTypography::ButtonConsequence.role())
          .width(Dimension::Pct(100.0));
        match self.kind {
          ButtonKind::Primary => words
            .color(ExtraColor::OnAccent.role())
            .opacity(CONSEQUENCE_ON_ACCENT_OPACITY),
          _ => words,
        }
      });
      let text = Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(label.variant(ExtraTypography::ButtonStacked.role()))
        .with_children(words);
      return text.into();
    }
    let label = label.variant(role);
    let label = if self.bounded {
      label
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
        .text_overflow(lurq::components::TextOverflow::Elipsis)
    } else {
      label
    };
    let label = if self.kind == ButtonKind::Select {
      label.flex(1.0).min_width(Dimension::Px(0.0))
    } else {
      label
    };
    label.into()
  }
}

impl From<ActionButton> for Element {
  fn from(button: ActionButton) -> Self {
    let color = if button.applied {
      PaletteColor::TextPrimary
    } else {
      button.kind.content()
    };
    let (_, glyph, _) = button.metrics();
    let mut frame = button.frame();
    if let Some(lead) = button.lead() {
      frame = frame.child(icon(lead, glyph, color.clone()).id(format!("{}-icon", button.id)));
    }
    let after = button
      .after
      .filter(|_| button.hold.is_none())
      .map(|after| icon(after, glyph, color.clone()));
    frame = button
      .kind
      .dressed(frame)
      .child(button.slot(color))
      .with_children(after);
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
    if let Some(on_focus) = button.on_focus {
      frame = frame.on_focus(move || on_focus());
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
      (Some(Hold::Shown), _) => inert(frame).cursor(CursorIcon::Default).into(),
      _ => inert(frame).opacity(DISABLED_OPACITY).into(),
    }
  }
}
