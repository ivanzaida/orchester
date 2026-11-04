//! A message's actions, in both chats (`06 · Message actions` `XPDAX`, ORC-706, as Claude desktop draws them): one
//! line of icon buttons with no box, stroke or fill around it, `control_height_sm` square and 2 apart, radius 6, each a
//! 14 glyph in `text_secondary`; under the pointer the `hover` fill and `text_primary`. Copy is always there, and a
//! message may offer more around it ([`super::toolbar`]). The line shows while the pointer is on the message or the
//! keyboard is on one of its buttons (with the focus ring), and keeps its room either way, so nothing moves when it
//! shows. A button's tooltip hangs under it while the pointer or the keyboard is on it: its label, or why it cannot be
//! used now. Copy puts the message's Markdown on the clipboard as it was kept, its secrets already redacted, without
//! its name and time; then for 2 s its glyph is a `check` in `success` and its tooltip says *Copied*, or a
//! `triangle-alert` in `danger` says *Couldn't copy*.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{PaletteColor, RadiusSize},
  },
  components::{Button, Placement, Row, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, dimension::Dimension},
};

use super::toolbar::{Action, HELD_CLASS, MessageExtras, ToolbarChoice};
use crate::{
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, IconSize},
  ui::{
    clipboard::Clipboard,
    icon::{Icon, icon},
    pressable::{Focus, Press, inert},
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

/// The classes of a Copy, or of a message's actions, by what came of its last press, for tests and QA.
pub const READY_CLASS: &str = "copy-ready";
pub const COPIED_CLASS: &str = "copy-copied";
pub const FAILED_CLASS: &str = "copy-failed";
/// The class of a message's actions while they show.
pub const SHOWN_CLASS: &str = "copy-shown";

/// Where a message's actions stand, which their tooltips hang from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Edge {
  /// Under an answer's text, at its start: a tooltip lines up with its button's start.
  Start,
  /// Under the person's bubble, at its end: a tooltip is centred under its button.
  End,
}

/// The actions of the message `id` that copy `markdown`, with `extras` around Copy, standing at `edge`; `hovered`
/// while the pointer is on the message. Their line's element id is `<id>-actions`, a button's `<id>-<action>`
/// (`copy`, `edit`, `rewind`, `fork`) and its tooltip's `<id>-<action>-tooltip`.
pub(super) fn message_actions(
  ctx: &mut Ctx,
  id: &str,
  (markdown, extras): (&Arc<str>, Option<MessageExtras>),
  (edge, hovered): (Edge, bool),
) -> Element {
  let clipboard = Clipboard::of(ctx);
  ctx.mount_keyed::<MessageActions>(
    &format!("message-actions-{id}"),
    MessageActionsProps {
      id: Arc::from(id),
      markdown: markdown.clone(),
      edge,
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
  edge: Edge,
  hovered: bool,
  clipboard: Clipboard,
  extras: Option<MessageExtras>,
}

impl DevtoolsInspectable for MessageActionsProps {}

struct MessageActions {
  copied: Signal<Copied>,
  reset: Timeout,
  /// The button the pointer is on.
  pointed: Signal<Option<Action>>,
  /// The button the keyboard is on.
  focused: Signal<Option<Action>>,
  /// Each action's button, by [`Action::index`], which its tooltip hangs from.
  anchors: [ElementRef; 4],
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
      pointed: ctx.signal(None),
      focused: ctx.signal(None),
      anchors: std::array::from_fn(|_| ctx.element_ref()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<MessageActionsProps>().clone();
    let copied = self.copied.get();
    let shown = props.hovered || self.tipped().is_some();
    let buttons: Vec<Element> = Action::ALL
      .into_iter()
      .filter_map(|action| self.action(ctx, &props, action))
      .collect();
    let line = Row::new()
      .id(format!("{}-actions", props.id))
      .height(Dimension::Px(ControlHeight::Sm.value()))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .class(copied.class())
      .with_children(buttons);
    if shown {
      line.class(SHOWN_CLASS)
    } else {
      line.opacity(0.0)
    }
  }
}

impl MessageActions {
  /// The button whose tooltip shows: the one the pointer or else the keyboard is on, or Copy while it says what came of
  /// its last press.
  fn tipped(&self) -> Option<Action> {
    let told = (self.copied.get() != Copied::Not).then_some(Action::Copy);
    self.pointed.get().or(self.focused.get()).or(told)
  }

  /// The button of `action`; `None` for one the message does not offer.
  fn action(&self, ctx: &Ctx, props: &MessageActionsProps, action: Action) -> Option<Element> {
    if action == Action::Copy {
      return Some(self.copy(ctx, props));
    }
    let choice = props.extras.as_ref()?.choice(action)?;
    Some(self.choice(props, action, choice))
  }

  /// Copy, its glyph and tooltip as its last press came out.
  fn copy(&self, ctx: &Ctx, props: &MessageActionsProps) -> Element {
    let (glyph, color, key) = match self.copied.get() {
      Copied::Not => (Icon::Copy, self.color(Action::Copy), "ui.chat.copyMessage"),
      Copied::Done => (Icon::Check, PaletteColor::Success, "ui.copied"),
      Copied::Failed => (Icon::TriangleAlert, PaletteColor::Danger, "ui.copyFailed"),
    };
    let words = ctx.t(key);
    let (copied, reset) = (self.copied.clone(), self.reset.clone());
    let (clipboard, markdown) = (props.clipboard.clone(), props.markdown.clone());
    let press = Press::run(move || {
      copied.set(Copied::of(&clipboard, &markdown));
      reset.restart();
    });
    let glyph = icon(glyph, IconSize::Md, color).id(format!("{}-copy-icon", props.id));
    let button = self.button(props, Action::Copy, (&words, &words), glyph);
    press.on(Focus::Keyboard.apply(button)).into()
  }

  /// The button of `action`, which `choice` makes: pressed with the pointer or the keyboard, or at half saying why
  /// it cannot be made now.
  fn choice(&self, props: &MessageActionsProps, action: Action, choice: &ToolbarChoice) -> Element {
    let glyph = icon(action.icon(), IconSize::Md, self.color(action));
    let label = &choice.label;
    match &choice.choose {
      Ok(choose) => {
        let button = self.button(props, action, (label, label), glyph);
        let pressed = Press::Run(choose.clone()).on(Focus::Keyboard.apply(button));
        pressed.describe("state", "enabled").into()
      }
      Err(why) => held(
        self.button(props, action, (label, why), glyph.opacity(DISABLED_OPACITY)),
        why,
      ),
    }
  }

  /// `text_primary` for the glyph of the button the pointer is on, else `text_secondary`.
  fn color(&self, action: Action) -> PaletteColor {
    if self.pointed.get() == Some(action) {
      PaletteColor::TextPrimary
    } else {
      PaletteColor::TextSecondary
    }
  }

  /// The button of `action` with `glyph`, named `name`, saying `tip` in its tooltip while that shows: the `hover` fill
  /// under the pointer; the line knows whether the pointer or the keyboard is on it. The tooltip is the button's own
  /// child, out of the layout and always laid out, seen only while it shows: a press under the pointer reaches the
  /// button it began on even when the pointer's arrival redraws the line in the same frame.
  fn button(
    &self,
    props: &MessageActionsProps,
    action: Action,
    (name, tip): (&Arc<str>, &Arc<str>),
    glyph: Text,
  ) -> Button {
    let anchor = self.anchors[action.index()].clone();
    let placement = match props.edge {
      Edge::Start => Placement::BottomStart,
      Edge::End => Placement::Bottom,
    };
    let tooltip = Tooltip::new(format!("{}-{}", props.id, action.key()), tip.clone()).hung(
      anchor.clone(),
      placement,
      self.tipped() == Some(action),
    );
    let (enter, leave) = (self.pointed.clone(), self.pointed.clone());
    let (focus, blur) = (self.focused.clone(), self.focused.clone());
    let side = Dimension::Px(ControlHeight::Sm.value());
    Button::empty()
      .id(format!("{}-{}", props.id, action.key()))
      .size(side, side)
      .flex_shrink(0.0)
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(RadiusSize::Md)
      .ref_element(anchor)
      .describe("a11y_name", name.clone())
      .cursor(CursorIcon::Pointer)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .active_style(Style::new().background(ExtraColor::Pressed.role()))
      .on_mouse_enter(move || enter.set(Some(action)))
      .on_mouse_leave(move || leave.update(|on| *on = on.filter(|on| *on != action)))
      .on_focus(move || focus.set(Some(action)))
      .on_blur(move || blur.update(|on| *on = on.filter(|on| *on != action)))
      .child(glyph)
      .child(tooltip)
  }
}

/// `button`, which cannot be pressed now (its glyph at half); `why` is its reason for tools.
fn held(button: Button, why: &Arc<str>) -> Element {
  inert(button)
    .class(HELD_CLASS)
    .describe("state", "unavailable")
    .describe("reason", why.clone())
    .into()
}
