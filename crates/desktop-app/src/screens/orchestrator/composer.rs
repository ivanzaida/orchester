//! The Orchestrator's composer (`Composer` `QQ2ZM` as the Orchestrator draws it, `ZRbgy`, `VP0Fm`): one box —
//! `surface_input` in a `border_strong` stroke (`border_focus` while the field has the focus), radius 10, 10 by 12
//! inside, 8 between the message and its row — whose row holds *Attach*, not built, faded with its reason in a
//! tooltip; the recipient pill *To: Orchestrator* (`waypoints`); what it acts with (a 12 `shield` and 11.5
//! `text_muted`, 8 in from its sides); and at the end the 28 `accent` square that sends (`arrow-up`), faded while
//! there is nothing to send. Under the box, 8 apart, the keys: Enter adds a line, Ctrl+Enter (Cmd+Enter on macOS)
//! sends. The draft stays until its message is kept; a failed send keeps it.

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

use super::View;
use crate::{
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    composer::recipient_pill,
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

/// What sending does now.
#[derive(Clone)]
pub(super) enum Sending {
  /// Nothing can be sent, and why: nothing is written, the Orchestrator has nothing to run on, or it answers.
  Held(Arc<str>),
  /// The message is being sent.
  Busy,
  Ready(Arc<dyn Fn() + Send + Sync>),
}

/// The box as a screen shows it.
pub(super) struct ComposerBox<'a> {
  pub(super) placeholder: Arc<str>,
  /// A new conversation's taller box.
  pub(super) start: bool,
  pub(super) sending: &'a Sending,
  pub(super) focused: &'a Signal<bool>,
}

/// The box and the keys under it.
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
    .child(attach(ctx))
    .child(recipient_pill(
      ctx,
      (&ctx.t("orchestrator.composer.recipient"), Icon::Waypoints),
      None,
    ))
    .child(permissions(ctx))
    .child(Spacer::new().flex(1.0))
    .child(send(ctx, composer.sending));
  let boxed = Column::new()
    .id("orchestrator-composer")
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Loose.role())
    .padding_horizontal(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(RadiusSize::Lg)
    .child(field)
    .child(bar);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(boxed)
}

/// The keys under the box: *Enter adds a line ·* [Ctrl Enter] *sends*.
pub(super) fn keys(ctx: &Ctx) -> Row {
  let keys = ctx.t(if cfg!(target_os = "macos") {
    "orchestrator.keys.cmdEnter"
  } else {
    "orchestrator.keys.ctrlEnter"
  });
  Row::new()
    .id("orchestrator-keys")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("orchestrator.composer.lines"))
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
    .child(key_cap("orchestrator-keys-send", &keys))
    .child(
      Text::new(&ctx.t("orchestrator.composer.sends"))
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
}

/// The message field: it grows with its lines up to [`MAX_LINES`].
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
    Sending::Ready(press) => Some(press.clone()),
    Sending::Held(_) | Sending::Busy => None,
  };
  let first = if composer.start { START_LINES } else { 1 };
  TextInput::styled(view.draft.clone(), resolve(PaletteColor::TextPrimary))
    .placeholder_style(resolve(PaletteColor::TextMuted))
    .caret_color(PaletteColor::TextPrimary)
    .id("orchestrator-input")
    .describe("a11y_name", composer.placeholder.clone())
    .textarea()
    .rows(first, MAX_LINES)
    .width(Dimension::Pct(100.0))
    .placeholder(&composer.placeholder)
    .ref_element(view.input.clone())
    .on_focus(move || gained.set(true))
    .on_blur(move || lost.set(false))
    .on_key_down(move |event: KeyboardEvent| sends(&event, press.as_deref()))
    .into()
}

/// Ctrl+Enter (Cmd+Enter) sends; Enter alone adds a line, as the field does by itself.
fn sends(event: &KeyboardEvent, press: Option<&(dyn Fn() + Send + Sync)>) {
  if event.key != "Enter" || !(event.ctrl || event.meta) || event.composing {
    return;
  }
  event.prevent_default();
  if let Some(press) = press {
    press();
  }
}

/// *Attach*: not built, faded, its reason in its tooltip.
fn attach(ctx: &mut Ctx) -> Element {
  let reason = ctx.t("orchestrator.composer.attach");
  let square = inert(Button::empty())
    .id("orchestrator-attach")
    .describe("state", "unavailable")
    .describe("reason", reason.clone())
    .size(Dimension::Px(ATTACH_SIZE), Dimension::Px(ATTACH_SIZE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .opacity(DISABLED_OPACITY)
    .child(icon(Icon::Paperclip, IconSize::Md, PaletteColor::TextSecondary));
  Tooltip::new("orchestrator-attach", reason).wrap(ctx, square)
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

/// The square that sends: faded with why while nothing can be sent, the loader while it sends.
fn send(ctx: &mut Ctx, sending: &Sending) -> Element {
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
  let (button, tip): (Button, Arc<str>) = match sending {
    Sending::Ready(press) => {
      let press = press.clone();
      let button = pressable(move || press())
        .describe("state", "enabled")
        .hovered_style(Style::new().background(PaletteColor::AccentHover));
      (button, label.clone())
    }
    Sending::Busy => (inert(Button::empty()).describe("state", "loading"), label.clone()),
    Sending::Held(why) => (
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
