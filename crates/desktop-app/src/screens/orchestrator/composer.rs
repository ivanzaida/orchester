//! The Orchestrator's composer (`Composer` `QQ2ZM` as the Orchestrator draws it, `ZRbgy`, `VP0Fm`): one box —
//! `surface_input` in a `border_strong` stroke (`border_focus` while the field has the focus), radius 10, 10 by 12
//! inside, 8 between the message and its row — whose row holds *Attach* (a 26 square with a `paperclip`, which opens
//! the system's picker); the recipient pill *To: Orchestrator* (`waypoints`); what it acts with (a 12 `shield` and
//! 11.5 `text_muted`, 8 in from its sides); and at the end the 28 `accent` square that sends (`arrow-up`), faded
//! while there is nothing to send. While the Orchestrator answers the composer stays usable (ORC-92): the square sends
//! after the answer, and a chevron beside it opens the other ways of sending ([`super::send_menu`]). The files attached
//! show as chips over the message (`ui::attachments::tray`); a paste of an image, files or long text attaches them,
//! and so does a drop on the box. Under the box, 8 apart, why the files last given were not attached, then the keys:
//! Enter adds a line, Ctrl+Enter (Cmd+Enter on macOS) sends, and while it answers Esc stops the answer (*Stop
//! response*), as in Claude Code: with the focus in the field, once nothing else open took the key. The draft and its
//! files stay until its message is kept; a failed send keeps them.

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
    composer::{message_scrollbar, recipient_pill},
    icon::{Icon, icon},
    kbd::key_cap,
    pressable::{inert, pressable},
    tooltip::Tooltip,
  },
};

/// The most lines the message grows to before its lines scroll; a new conversation's box starts at three
/// (`B8fEg`, 58 high, in `ZRbgy`).
const MAX_LINES: usize = 6;
const START_LINES: usize = 3;
/// *Attach*'s square (`hprBf`).
const ATTACH_SIZE: f32 = 26.0;
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
  /// A new conversation's taller box.
  pub(super) start: bool,
  pub(super) sending: &'a Sending,
  pub(super) focused: &'a Signal<bool>,
  /// Stops the answer under way (Esc), while there is one.
  pub(super) stop: Option<Press>,
}

/// The box, and why the files last given were not attached under it.
pub(super) fn composer(ctx: &mut Ctx, view: &View, composer: &ComposerBox<'_>) -> Column {
  let stroke = if composer.focused.get() {
    PaletteColor::BorderFocus
  } else {
    PaletteColor::BorderStrong
  };
  let field = field(ctx, view, composer);
  let bar = Row::new()
    .id("orchestrator-composer-bar")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(attach(ctx, view))
    .child(recipient_pill(
      ctx,
      (&ctx.t("orchestrator.composer.recipient"), Icon::Waypoints),
      None,
    ))
    .child(permissions(ctx))
    .child(Spacer::new().flex(1.0))
    .child(send(ctx, view, composer.sending));
  let draft = view.draft.clone();
  let put_back: PutBack = Arc::new(move |text: String| draft.update(|words| tray::put_back_into(words, &text)));
  let attached = tray::tray(ctx, ATTACHMENTS, &view.attached, &put_back);
  let boxed = Column::new()
    .id("orchestrator-composer")
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Loose.role())
    .padding_horizontal(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceInput)
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

/// The keys under the box: *Enter adds a line ·* [Ctrl Enter] *sends*, and while the Orchestrator `answering`,
/// *· [Esc] stops the answer*.
pub(super) fn keys(ctx: &Ctx, answering: bool) -> Row {
  let keys = ctx.t(if cfg!(target_os = "macos") {
    "orchestrator.keys.cmdEnter"
  } else {
    "orchestrator.keys.ctrlEnter"
  });
  let caption = |key: &str| Text::new(&ctx.t(key)).variant(TypographyStyle::Caption).nowrap();
  let stops = answering.then(|| {
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(key_cap("orchestrator-keys-stop", &ctx.t("orchestrator.keys.esc")))
      .child(caption("orchestrator.composer.stops"))
  });
  Row::new()
    .id("orchestrator-keys")
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(caption("orchestrator.composer.lines"))
    .child(key_cap("orchestrator-keys-send", &keys))
    .child(caption(if answering {
      "orchestrator.composer.sendsAfter"
    } else {
      "orchestrator.composer.sends"
    }))
    .with_children(stops)
}

/// The message field: it grows with its lines up to [`MAX_LINES`]; a paste of an image, files or long text attaches
/// them instead of pasting words.
fn field(ctx: &Ctx, view: &View, composer: &ComposerBox<'_>) -> Element {
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
  let first = if composer.start { START_LINES } else { 1 };
  TextInput::styled(view.draft.clone(), resolve(PaletteColor::TextPrimary))
    .placeholder_style(resolve(PaletteColor::TextMuted))
    .caret_color(PaletteColor::TextPrimary)
    .id("orchestrator-input")
    .describe("a11y_name", composer.placeholder.clone())
    .textarea()
    .rows(first, MAX_LINES)
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
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(Icon::Paperclip, IconSize::Md, PaletteColor::TextSecondary));
  Tooltip::new("orchestrator-attach", ctx.t("attachments.attachHint")).wrap(ctx, square)
}

/// What the Orchestrator acts with, and in its tooltip how its actions are checked.
fn permissions(ctx: &mut Ctx) -> Element {
  let label = Row::new()
    .id("orchestrator-permissions")
    .height(Dimension::Px(ControlHeight::Sm.value()))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Shield, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("orchestrator.composer.permissions"))
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    );
  Tooltip::new(
    "orchestrator-permissions",
    ctx.t("orchestrator.composer.permissionsHint"),
  )
  .wrap(ctx, label)
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
    .background(PaletteColor::Accent)
    .rounded(RadiusSize::Md)
    .child(face);
  Tooltip::new("orchestrator-send", tip).wrap(ctx, button)
}
