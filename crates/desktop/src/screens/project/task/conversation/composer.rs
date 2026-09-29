//! The composer (`C3nu0z`): the message being written on the surface fill, with *Attach* and
//! *Send* under it. Enter adds a newline and Ctrl+Enter (Cmd+Enter on macOS) sends, as the design
//! asks (`docs/design/orchester-workspace.md`, "Task conversation"); Send is disabled while the
//! message is empty or on its way. Attachments do not exist yet, so Attach is drawn unavailable. A
//! run that ended takes no messages: the composer says so instead of taking text.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::keyboard_event::KeyboardEvent,
    theme::{BorderSize, PaletteColor, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{button::ActionButton, field::styled_input, icon::Icon},
};

/// Height of Attach and Send (`lUFjz`, `EEQJC`), and the most lines the message grows to before it
/// scrolls.
const BUTTON_HEIGHT: f32 = 30.0;
const MAX_LINES: usize = 6;

pub(super) struct Composer<'a> {
  pub(super) draft: &'a Signal<String>,
  pub(super) focused: &'a Signal<bool>,
  pub(super) input: &'a ElementRef,
  /// Sends the draft; `None` while it is empty or a message is on its way.
  pub(super) send: Option<Arc<dyn Fn() + Send + Sync>>,
  /// A message is on its way.
  pub(super) sending: bool,
  /// The run takes messages; one that ended does not.
  pub(super) open: bool,
  /// Sending explicitly continues a failed run, rather than only queuing a message.
  pub(super) continues_failed: bool,
}

pub(super) fn composer(ctx: &Ctx, composer: Composer<'_>) -> Element {
  let border = if composer.focused.get() {
    Border::inside(BorderSize::Md, PaletteColor::BorderFocus)
  } else {
    Border::inside(BorderSize::Sm, PaletteColor::Border)
  };
  let field: Element = if composer.open {
    input(ctx, &composer)
  } else {
    Text::new(&ctx.t("conversation.composer.closed"))
      .id("conversation-input-closed")
      .variant(ExtraTypography::ReviewNote.role())
      .color(PaletteColor::TextMuted)
      .width(Dimension::Pct(100.0))
      .into()
  };
  let send_label = if composer.sending {
    ctx.t("conversation.composer.sending")
  } else if composer.continues_failed {
    ctx.t("conversation.composer.sendAndContinue")
  } else {
    ctx.t("conversation.composer.send")
  };
  let mut send = ActionButton::new(send_label)
    .icon(Icon::ArrowUp)
    .primary()
    .in_form()
    .height(Dimension::Px(BUTTON_HEIGHT))
    .id("conversation-send");
  send = match composer.send {
    Some(handler) => send.on_click(move || handler()),
    None => send.disabled(),
  };
  let attach = ActionButton::new(ctx.t("conversation.composer.attach"))
    .icon(Icon::Paperclip)
    .height(Dimension::Px(BUTTON_HEIGHT))
    .id("conversation-attach");
  Column::new()
    .id("conversation-composer")
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Group.role())
    .background(ExtraColor::Surface.role())
    .border(border)
    .rounded(ExtraRadius::Notice.role())
    .child(field)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Inline.role())
        .align_items(Alignment::Center)
        .child(attach.render(ctx))
        .child(Spacer::new().flex(1.0))
        .child(send.render(ctx)),
    )
    .into()
}

/// The message field: it grows with its lines up to [`MAX_LINES`]; Ctrl+Enter or Cmd+Enter sends.
fn input(ctx: &Ctx, composer: &Composer<'_>) -> Element {
  let (gained, lost) = (composer.focused.clone(), composer.focused.clone());
  let send = composer.send.clone();
  // `UXzbp` is a 12px normal field line; ReviewNote's 1.6 leading inflated the empty composer.
  styled_input(ctx, composer.draft.clone(), TypographyStyle::FieldLabel)
    .id("conversation-input")
    .textarea()
    .rows(1, MAX_LINES)
    .width(Dimension::Pct(100.0))
    .placeholder(&ctx.t("conversation.composer.placeholder"))
    .ref_element(composer.input.clone())
    .on_focus(move || gained.set(true))
    .on_blur(move || lost.set(false))
    .on_key_down(move |event: KeyboardEvent| {
      if event.key == "Enter" && (event.ctrl || event.meta) {
        // The shortcut sends; it never also adds a newline, even when there is nothing to send.
        event.prevent_default();
        if let Some(send) = &send {
          send();
        }
      }
    })
    .into()
}
