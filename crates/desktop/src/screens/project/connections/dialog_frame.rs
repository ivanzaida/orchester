//! The frame shared by the connection dialogs (`IAN4P` in 04.02, `d8yPD5` in 04.06): header
//! with title, owning project and Close (`osMan`), fields scrolling beside the scroll gutter
//! (`aqguY` in 04.E `yRb2k`), and Cancel and Save connection with why saving is not possible yet
//! (`InHni`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::ElementRef,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use super::DialogContext;
use crate::{
  messages::Message,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    backdrop::backdrop,
    button::{ActionButton, SKIPPED_BY_TAB},
    form::{Submit, form},
    icon::{Icon, IconSize, icon},
    message_text,
  },
};

/// Dialog, header, action bar and button sizes (`IAN4P`, `osMan`, `InHni`, `I3629f`).
const DIALOG_WIDTH: f32 = 800.0;
const DIALOG_HEIGHT: f32 = 832.0;
const HEADER_HEIGHT: f32 = 82.0;
const ACTIONS_HEIGHT: f32 = 76.0;
pub(super) const BUTTON_HEIGHT: f32 = 36.0;

/// What the action bar shows.
pub(super) struct SaveBar {
  /// The save runs.
  pub saving: bool,
  /// Why saving is not possible, or why the last save failed.
  pub reason: Option<Message>,
  pub submit: Submit,
}

/// The dialog over its backdrop, as a form that submits with `bar.submit`. `frame` is focused
/// when the dialog opens, so that Tab starts at its first control; the design shows no field
/// focused, so no field takes the focus.
pub(super) fn dialog(
  ctx: &mut Ctx,
  frame: &ElementRef,
  title: &str,
  context: &DialogContext,
  fields: Element,
  bar: SaveBar,
) -> Element {
  let submit = bar.submit.clone();
  let dialog = Column::new()
    .id("connection-dialog")
    .ref_element(frame.clone())
    .focusable(true)
    .tab_index(SKIPPED_BY_TAB)
    .width(Dimension::Px(DIALOG_WIDTH))
    .height(Dimension::Px(DIALOG_HEIGHT))
    .background(ExtraColor::Surface.role())
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Lg)
    .child(header(ctx, title, context))
    .child(
      ScrollVertical::new(padded(fields))
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(gutter_scrollbar()),
    )
    .child(actions(ctx, context, bar, CONNECTION_SAVE, None));
  backdrop(ctx, form(submit, dialog))
}

/// The save button of an action bar: its id and its label.
pub(super) type SaveButton = (&'static str, &'static str);

const CONNECTION_SAVE: SaveButton = ("connection-save", "connections.dialog.save");

/// The form's padding: 28 all round but 16 at the right, beside the 12px gutter (`cJGKL`).
pub(super) fn padded(fields: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Rail.role())
    .padding_left(ExtraSpacing::Rail.role())
    .padding_right(SpacingSize::Md)
    .overflow_visible()
    .child(fields)
    .into()
}

pub(super) fn header(ctx: &Ctx, title: &str, context: &DialogContext) -> Element {
  let close = context.close.clone();
  let owner = ctx.t_args(
    "connections.dialog.owner",
    [("project", context.project.project.name().as_str())],
  );
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADER_HEIGHT))
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(SpacingSize::Sm)
        .child(Text::new(title).variant(ExtraTypography::DialogTitle.role()))
        .child(Text::new(&owner).variant(TypographyStyle::Caption)),
    )
    .child(
      ActionButton::new(ctx.t("common.close"))
        .icon(Icon::X)
        .id("connection-dialog-close")
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(move || close())
        .render(ctx),
    )
    .into()
}

/// Header of a dialog that explains itself instead of naming its project (`vnIxL` in 04.H6): the
/// title over `explanation`, without Close; Cancel closes it.
pub(super) fn explained_header(title: &str, explanation: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(title).variant(ExtraTypography::DialogTitle.role()))
    .child(Text::new(explanation).variant(ExtraTypography::Explanation.role()))
    .into()
}

/// The action bar: why saving is not possible, else `note` (`FU7e7` in 04.H6), then Cancel and the
/// save button.
pub(super) fn actions(
  ctx: &Ctx,
  context: &DialogContext,
  bar: SaveBar,
  (id, save): SaveButton,
  note: Option<&str>,
) -> Element {
  let label = if bar.saving { "connections.dialog.saving" } else { save };
  let save_button = ActionButton::new(ctx.t(label))
    .primary()
    .icon(Icon::Check)
    .id(id)
    .in_form()
    .height(Dimension::Px(BUTTON_HEIGHT));
  let save_button = match bar.submit {
    Some(submit) => save_button.on_click(move || submit()),
    // Enabled once the reason shown beside it is resolved.
    None => save_button.on_click(|| {}).enabled(false),
  };
  let close = context.close.clone();
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ACTIONS_HEIGHT))
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .with_children(match (bar.reason, note) {
      (Some(reason), _) => Some(save_reason(ctx, &reason)),
      (None, Some(note)) => Some(Text::new(note).variant(TypographyStyle::Caption).into()),
      (None, None) => None,
    })
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new(ctx.t("common.cancel"))
        .icon(Icon::X)
        .id("connection-cancel")
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(move || close())
        .render(ctx),
    )
    .child(save_button.render(ctx))
    .into()
}

fn save_reason(ctx: &Ctx, reason: &Message) -> Element {
  Row::new()
    .id("connection-save-reason")
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(icon(Icon::AlertCircle, IconSize::Small, PaletteColor::TextSecondary))
    .child(Text::new(&message_text(ctx, reason)).variant(TypographyStyle::Caption))
    .into()
}

/// A suggested connection name from `key` with `args`, numbered with `numbered` when the plain
/// name is taken by another connection of the project.
pub(super) fn suggested_name(
  ctx: &Ctx,
  context: &DialogContext,
  key: &str,
  numbered: &str,
  args: &[(&str, &str)],
) -> String {
  let taken = |name: &str| context.taken.iter().any(|taken| taken.as_str() == name);
  let plain = ctx.t_args(key, args.iter().copied());
  if !taken(&plain) {
    return plain.to_string();
  }
  (2_usize..)
    .map(|number| {
      let number = number.to_string();
      let args = args.iter().copied().chain([("number", number.as_str())]);
      ctx.t_args(numbered, args).to_string()
    })
    .find(|name| !taken(name))
    .expect("only finitely many connection names are taken")
}

/// Why saving is not possible, or why the last save failed.
pub(super) fn save_reason_of<T>(draft: &Result<T, Message>, error: Option<Message>) -> Option<Message> {
  match (draft, error) {
    (_, Some(error)) => Some(error),
    (Err(reason), None) => Some(reason.clone()),
    (Ok(_), None) => None,
  }
}
