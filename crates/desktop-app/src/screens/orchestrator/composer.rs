//! The Orchestrator's composer (`Composer` `Nv6kV` in `01 Conversation` `wRSL4`): one box — `surface_input` in a
//! `border_strong` stroke (`border_focus` while the field has the focus), radius 10, 10 over it, 10 at its end, 8
//! under it and 12 at its start, 10 between the message and its bar — whose bar holds, 6 apart, *Attach* (a 24 square
//! with a `paperclip`, which opens the system's picker); the runs-on chip ([`super::runs_on`]); *Ctrl Enter* in Geist
//! Mono 11 `text_muted`; and the 28 `accent` square that sends (`arrow-up`), faded while there is nothing to send.
//! While the Orchestrator answers the composer stays usable (ORC-92): the square sends after the answer, and a chevron
//! beside it opens the other ways of sending ([`super::send_menu`]); Esc stops the answer. Until the Orchestrator is
//! set up (`02 Not set up` `Wd5CL`) the box is on `surface_panel` in a `border` stroke, says it can't answer yet, the
//! chip reads *Not set up* and *Set up the Orchestrator* takes the keys' and the square's place. The files attached
//! show as chips over the message (`ui::attachments::tray`); a paste of an image, files or long text attaches them,
//! and so does a drop on the box; why the files last given were not attached shows under it. The draft and its files
//! stay until its message is kept; a failed send keeps them.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Spacer, Text, TextInput},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  View,
  send_menu::{self, Press, Ways},
};
use crate::{
  services::attachment_picker::AttachmentPicker,
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    attachments::tray::{self, PutBack},
    composer::message_scrollbar,
    icon::{Icon, icon},
    pressable::{inert, pressable},
    tooltip::Tooltip,
  },
};

/// The most lines the message grows to before its lines scroll.
const MAX_LINES: usize = 6;
/// *Attach*'s square (`PkWbZ`).
const ATTACH_SIZE: f32 = 24.0;
/// What names the composer's attachments, in tools and tests.
const ATTACHMENTS: &str = "orchestrator-composer";

/// What sending does now.
#[derive(Clone)]
pub(super) enum Sending {
  /// Nothing can be sent, and why: the Orchestrator has nothing to run on.
  Held(Arc<str>),
  /// Nothing is written, as `why` says, as far as the screen last drew it. The keys still send: the words typed
  /// just before them may not be drawn yet, and sending reads the draft as it is then. While the Orchestrator answers
  /// (`answering`) the faded button is the split one.
  Empty {
    why: Arc<str>,
    press: Press,
    answering: bool,
  },
  /// The message is being sent, or the files given are still being read.
  Busy,
  Ready(Press),
  /// The Orchestrator answers: the square sends after the answer, and the send menu offers the other ways.
  WhileAnswering(Arc<Ways>),
}

/// The box as a screen shows it.
pub(super) struct ComposerBox<'a> {
  pub(super) placeholder: Arc<str>,
  pub(super) sending: &'a Sending,
  pub(super) focused: &'a Signal<bool>,
  /// Stops the answer under way (Esc), while there is one.
  pub(super) stop: Option<Press>,
  /// The runs-on chip.
  pub(super) runs_on: Element,
  /// *Set up the Orchestrator*, while it is not set up.
  pub(super) set_up: Option<Element>,
}

/// The box, and why the files last given were not attached under it.
pub(super) fn composer(ctx: &mut Ctx, view: &View, composer: ComposerBox<'_>) -> Column {
  let not_set_up = composer.set_up.is_some();
  let (fill, stroke) = match (not_set_up, composer.focused.get()) {
    (true, _) => (PaletteColor::SurfacePanel, PaletteColor::Border),
    (false, true) => (PaletteColor::SurfaceInput, PaletteColor::BorderFocus),
    (false, false) => (PaletteColor::SurfaceInput, PaletteColor::BorderStrong),
  };
  let field = field(ctx, view, &composer, not_set_up);
  let end: Vec<Element> = match composer.set_up {
    Some(set_up) => vec![set_up],
    None => vec![keys(ctx).into(), send(ctx, view, composer.sending)],
  };
  let bar = Row::new()
    .id("orchestrator-composer-bar")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(attach(ctx, view))
    .child(composer.runs_on)
    .child(Spacer::new().flex(1.0))
    .with_children(end);
  let draft = view.draft.clone();
  let put_back: PutBack = Arc::new(move |text: String| draft.update(|words| tray::put_back_into(words, &text)));
  let attached = tray::tray(ctx, ATTACHMENTS, &view.attached, &put_back);
  let boxed = Column::new()
    .id("orchestrator-composer")
    .describe("set_up", (!not_set_up).to_string())
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Loose.role())
        .right(ExtraSpacing::Loose.role())
        .bottom(SpacingSize::Sm)
        .left(SpacingSize::Md),
    )
    .spacing(ExtraSpacing::Loose.role())
    .background(fill)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(RadiusSize::Lg)
    .with_children(attached)
    .child(field)
    .child(bar);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(tray::drop_target(boxed, &view.attached))
    .with_children(tray::refusals(ctx, ATTACHMENTS, &view.attached))
}

/// *Ctrl Enter* (*⌘ Enter* on macOS), the keys that send.
fn keys(ctx: &Ctx) -> Text {
  Text::new(&ctx.t(if cfg!(target_os = "macos") {
    "orchestrator.keys.cmdEnter"
  } else {
    "orchestrator.keys.ctrlEnter"
  }))
  .id("orchestrator-keys")
  .variant(ExtraTypography::MonoNote.role())
  .nowrap()
}

/// The message field: it grows with its lines up to [`MAX_LINES`]; a paste of an image, files or long text attaches
/// them instead of pasting words. Until the Orchestrator is set up its placeholder, saying so, is in `text_secondary`.
fn field(ctx: &Ctx, view: &View, composer: &ComposerBox<'_>, not_set_up: bool) -> Element {
  let theme = ctx.theme();
  let resolve = |color| {
    FormTextRole {
      typography: TypographyStyle::Body,
      color,
    }
    .resolve(&theme.typography(), &theme.palette())
  };
  let (gained, lost) = (composer.focused.clone(), composer.focused.clone());
  let press = match composer.sending {
    Sending::Ready(press) | Sending::Empty { press, .. } => Some(press.clone()),
    Sending::WhileAnswering(ways) => Some(ways.after_answer.clone()),
    Sending::Held(_) | Sending::Busy => None,
  };
  let keys = Keys {
    press,
    stop: composer.stop.clone(),
    menu: view.menu.clone(),
    focused: composer.focused.clone(),
  };
  let placeholder = if not_set_up {
    PaletteColor::TextSecondary
  } else {
    PaletteColor::TextMuted
  };
  TextInput::styled(view.draft.clone(), resolve(PaletteColor::TextPrimary))
    .placeholder_style(resolve(placeholder))
    .caret_color(PaletteColor::TextPrimary)
    .id("orchestrator-input")
    .describe("a11y_name", composer.placeholder.clone())
    .textarea()
    .rows(1, MAX_LINES)
    .scrollbar(message_scrollbar(ctx))
    .width(Dimension::Pct(100.0))
    .placeholder(&composer.placeholder)
    .ref_element(view.input.clone())
    .on_focus(move || gained.set(true))
    .on_blur(move || lost.set(false))
    .on_key_down(move |event: KeyboardEvent| keys.down(&event))
    .on_paste(view.attached.on_paste())
    .into()
}

/// What the field's keys do. lurq hands a key to every node that takes keys, from the window's root down, so each key
/// is the field's only while it has the focus and nothing before it took the key.
struct Keys {
  press: Option<Press>,
  stop: Option<Press>,
  menu: Signal<bool>,
  focused: Signal<bool>,
}

impl Keys {
  /// Ctrl+Enter (Cmd+Enter) sends; Enter alone adds a line, as the field does by itself. Esc closes the send menu,
  /// else stops the answer under way.
  fn down(&self, event: &KeyboardEvent) {
    if event.key == "Escape" {
      if event.default_prevented() || !self.focused.get_untracked() {
        return;
      }
      if self.menu.get_untracked() {
        self.menu.set(false);
        event.prevent_default();
      } else if let Some(stop) = &self.stop {
        stop();
        event.prevent_default();
      }
      return;
    }
    if event.key != "Enter" || !(event.ctrl || event.meta) || event.composing {
      return;
    }
    event.prevent_default();
    if let Some(press) = &self.press {
      press();
    }
  }
}

/// *Attach*: the system's picker, whose files are attached; its tooltip says what can be attached and how else.
fn attach(ctx: &mut Ctx, view: &View) -> Element {
  let label = ctx.t("attachments.attach");
  let press = view
    .attached
    .on_attach(AttachmentPicker::of(ctx), ctx.t("attachments.pickerTitle"));
  let square = pressable(press)
    .id("orchestrator-attach")
    .describe("state", "enabled")
    .describe("a11y_name", label)
    .size(Dimension::Px(ATTACH_SIZE), Dimension::Px(ATTACH_SIZE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(Icon::Paperclip, IconSize::Md, PaletteColor::TextMuted));
  Tooltip::new("orchestrator-attach", ctx.t("attachments.attachHint")).wrap(ctx, square)
}

/// The square that sends: faded with why while nothing can be sent, the loader while it sends; split with the send
/// menu while the Orchestrator answers.
fn send(ctx: &mut Ctx, view: &View, sending: &Sending) -> Element {
  let glyph = if matches!(sending, Sending::Busy) {
    Icon::LoaderCircle
  } else {
    Icon::ArrowUp
  };
  let side = Dimension::Px(ControlHeight::Md.value());
  let face = Row::new()
    .size(side, side)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(glyph, IconSize::Md, ExtraColor::OnAccent.role()));
  let label = ctx.t("orchestrator.composer.send");
  let menu = (&view.menu, &view.menu_anchor);
  let (button, tip): (Button, Arc<str>) = match sending {
    Sending::WhileAnswering(ways) => {
      let empty = ctx.t("orchestrator.composer.sendEmpty");
      return send_menu::split(ctx, (Some(ways), &empty), menu);
    }
    Sending::Empty {
      why, answering: true, ..
    } => return send_menu::split(ctx, (None, why), menu),
    Sending::Ready(press) => {
      let press = press.clone();
      let button = pressable(move || press())
        .describe("state", "enabled")
        .hovered_style(Style::new().background(PaletteColor::AccentHover));
      (button, label.clone())
    }
    Sending::Busy => (inert(Button::empty()).describe("state", "loading"), label.clone()),
    Sending::Held(why) | Sending::Empty { why, .. } => (
      inert(Button::empty())
        .describe("state", "disabled")
        .opacity(DISABLED_OPACITY),
      why.clone(),
    ),
  };
  let button = button
    .id("orchestrator-send")
    .describe("a11y_name", label)
    .flex_shrink(0.0)
    .background(PaletteColor::Accent)
    .rounded(RadiusSize::Md)
    .child(face);
  Tooltip::new("orchestrator-send", tip).wrap(ctx, button)
}
