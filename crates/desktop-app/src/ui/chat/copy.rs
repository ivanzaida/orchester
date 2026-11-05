//! Copy on a message of a chat, the Orchestrator's and a run's Conversation alike (`Message actions` `EXtmO` in 07
//! `GgXmd`, `Lx2Cs` in 09 `w7TIt`, its states in 09e `Ezec2`, ORC-323): a toolbar `control_height_sm` square on
//! `surface_raised` inside a `border_strong` stroke, radius 6, 1 inside it, holding the small icon button with `copy`
//! in `text_primary`, the `hover` fill under the pointer. It shows while the pointer is on the message or the keyboard
//! is on its button, and takes its room either way, so nothing moves when it shows. Under the pointer or the keyboard,
//! its tooltip *Copy message* shows 6 to its left, in room it also keeps. A press puts the message's Markdown on the
//! clipboard as it was kept,
//! its secrets already redacted, without its name and time; then for 2 s the glyph is a `check` in `success` and the
//! tooltip says *Copied*, or a `triangle-alert` in `danger` says *Couldn't copy*.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Button, Row},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};

use super::toolbar::MessageExtras;
use crate::{
  theme::{ControlHeight, ExtraColor, ExtraSpacing, IconSize},
  ui::{
    clipboard::Clipboard,
    icon::{Icon, icon},
    pressable::{Focus, Press},
    tooltip::Tooltip,
  },
};

/// How long Copy says what happened before it offers to copy again.
pub(super) const COPY_FEEDBACK: Duration = Duration::from_secs(2);

/// What came of the last press of a Copy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Copied {
  #[default]
  Not,
  Done,
  Failed,
}

impl DevtoolsInspectable for Copied {}

impl Copied {
  /// What a press of a Copy with `clipboard` that puts `text` there came to.
  pub(super) fn of(clipboard: &Clipboard, text: &str) -> Self {
    if clipboard.copy(text) { Self::Done } else { Self::Failed }
  }

  /// The class tools and tests read the state by.
  pub(super) const fn class(self) -> &'static str {
    match self {
      Self::Not => READY_CLASS,
      Self::Done => COPIED_CLASS,
      Self::Failed => FAILED_CLASS,
    }
  }
}

/// The classes of a Copy, or of a message's toolbar, by what came of its last press, for tests and QA.
pub const READY_CLASS: &str = "copy-ready";
pub const COPIED_CLASS: &str = "copy-copied";
pub const FAILED_CLASS: &str = "copy-failed";
/// The class of a message's toolbar while it shows.
pub const SHOWN_CLASS: &str = "copy-shown";

/// The toolbar of the message `id` that copies `markdown`; `hovered` while the pointer is on the message. Its element
/// id is `<id>-actions`, its button's `<id>-copy`.
pub fn message_actions(ctx: &mut Ctx, id: &str, markdown: &Arc<str>, hovered: bool) -> Element {
  message_actions_with(ctx, id, (markdown, None), hovered)
}

/// [`message_actions`] with `extras` around Copy ([`super::toolbar`]): *Edit* before it and `…` after it. It also
/// shows while the `…` menu is open.
pub fn message_actions_with(
  ctx: &mut Ctx,
  id: &str,
  (markdown, extras): (&Arc<str>, Option<MessageExtras>),
  hovered: bool,
) -> Element {
  let clipboard = Clipboard::of(ctx);
  ctx.mount_keyed::<MessageActions>(
    &format!("message-actions-{id}"),
    MessageActionsProps {
      id: Arc::from(id),
      markdown: markdown.clone(),
      hovered,
      clipboard,
      extras,
    },
  )
}

#[derive(Clone, PartialEq)]
struct MessageActionsProps {
  id: Arc<str>,
  markdown: Arc<str>,
  hovered: bool,
  clipboard: Clipboard,
  extras: Option<MessageExtras>,
}

impl DevtoolsInspectable for MessageActionsProps {}

struct MessageActions {
  copied: Signal<Copied>,
  reset: Timeout,
  /// The pointer is on the button.
  pointed: Signal<bool>,
  /// The keyboard is on the button.
  focused: Signal<bool>,
  /// The `…` menu is open, under its button.
  menu: Signal<bool>,
  more: ElementRef,
}

impl Component for MessageActions {
  type Props = MessageActionsProps;

  fn create(ctx: &mut Ctx) -> Self {
    let copied = ctx.signal(Copied::Not);
    let back = copied.clone();
    let reset = ctx.create_timeout(COPY_FEEDBACK, move || back.set(Copied::Not));
    Self {
      copied,
      reset,
      pointed: ctx.signal(false),
      focused: ctx.signal(false),
      menu: ctx.signal(false),
      more: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<MessageActionsProps>().clone();
    let copied = self.copied.get();
    let (glyph, color, key) = match copied {
      Copied::Not => (Icon::Copy, PaletteColor::TextPrimary, "ui.chat.copyMessage"),
      Copied::Done => (Icon::Check, PaletteColor::Success, "ui.copied"),
      Copied::Failed => (Icon::TriangleAlert, PaletteColor::Danger, "ui.copyFailed"),
    };
    let words = ctx.t(key);
    let near = self.pointed.get() || self.focused.get();
    let said = near || copied != Copied::Not;
    let shown = props.hovered || said || self.menu.get();
    // The tooltip is always laid out, unseen until it says something: a press under the pointer reaches the button
    // it began on even when the pointer's arrival redraws the toolbar in the same frame.
    let tip = Tooltip::new(format!("{}-copy", props.id), words.clone()).beside();
    let copy = self.button(&props, (glyph, color), &words);
    let toolbar = Row::new()
      .id(format!("{}-actions", props.id))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .class(copied.class())
      .child(if said { tip } else { tip.opacity(0.0) })
      .child(self.toolbox(ctx, &props, copy));
    if shown {
      toolbar.class(SHOWN_CLASS)
    } else {
      toolbar.opacity(0.0)
    }
  }
}

impl MessageActions {
  /// The toolbar's box (`Fh2UW`): Copy alone in its square, or with the extras around it, 1 apart.
  fn toolbox(&self, ctx: &mut Ctx, props: &MessageActionsProps, copy: Button) -> Row {
    let body = Row::new()
      .height(Dimension::Px(ControlHeight::Sm.value()))
      .padding(ExtraSpacing::Hair.role())
      .spacing(ExtraSpacing::Hair.role())
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .rounded(RadiusSize::Md);
    let Some(extras) = &props.extras else {
      return body.width(Dimension::Px(ControlHeight::Sm.value())).child(copy);
    };
    let edit = extras
      .edit
      .as_ref()
      .map(|edit| super::toolbar::edit_button(ctx, &props.id, edit));
    let more = extras
      .more
      .as_ref()
      .map(|more| super::toolbar::more_button(ctx, &props.id, more, (&self.menu, &self.more)));
    body
      .with_children(edit)
      .child(copy.width(Dimension::Px(ControlHeight::Helper.value())))
      .with_children(more)
  }

  fn button(&self, props: &MessageActionsProps, (glyph, color): (Icon, PaletteColor), words: &Arc<str>) -> Button {
    let (copied, reset) = (self.copied.clone(), self.reset.clone());
    let (clipboard, markdown) = (props.clipboard.clone(), props.markdown.clone());
    let press = Press::run(move || {
      copied.set(Copied::of(&clipboard, &markdown));
      reset.restart();
    });
    let (enter, leave) = (self.pointed.clone(), self.pointed.clone());
    let (focus, blur) = (self.focused.clone(), self.focused.clone());
    let button = Button::empty()
      .id(format!("{}-copy", props.id))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(RadiusSize::Md)
      .describe("a11y_name", words.clone())
      .cursor(CursorIcon::Pointer)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .active_style(Style::new().background(ExtraColor::Pressed.role()))
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .on_focus(move || focus.set(true))
      .on_blur(move || blur.set(false))
      .child(icon(glyph, IconSize::Md, color).id(format!("{}-copy-icon", props.id)));
    press.on(Focus::Keyboard.apply(button))
  }
}
