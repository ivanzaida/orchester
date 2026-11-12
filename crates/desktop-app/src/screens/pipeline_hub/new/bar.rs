//! The pinned bar of a new pipeline's panel (`Actions` `vtZ9C` in P14 `Rszkt`): *Create v1* and *Cancel*, and at its end
//! why *Create v1* is off (the reason also in its tooltip, as *Save*'s is), each step it names a link that opens the
//! step in the panel, or what it saves. Above it, in place, what a
//! refused create says: nothing was stored and everything typed stays.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::StepKey;
use orchester_workspace::messages::Message;

use super::reason::{Piece, Reason};

use crate::{
  screens::team_hub::bar::{bar_ending, bar_saying},
  theme::{HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    pressable::{Focus, Press},
    tooltip::Tooltip,
  },
};

/// What *Create v1* can do now.
pub(super) enum Creating {
  /// It cannot create the pipeline as it stands, and why.
  Off(Reason),
  /// It creates it, which the bar says.
  Ready(Press, Arc<str>),
  /// It is storing it, which the bar says.
  Busy(Arc<str>),
}

/// The bar: *Create v1*, *Cancel* (off while the pipeline is stored) and what *Create v1* does or waits for; a step the
/// reason names opens with the press `opens` gives for it.
pub(super) fn bar(ctx: &mut Ctx, (creating, cancel): (Creating, Press), opens: &dyn Fn(&StepKey) -> Press) -> Element {
  let create = ActionButton::new(
    "pipeline-new-create",
    ButtonKind::Primary,
    ctx.t("pipeline_new.bar.create"),
  )
  .icon(Icon::Plus)
  .pinned();
  let cancelling = ActionButton::new(
    "pipeline-new-cancel",
    ButtonKind::Secondary,
    ctx.t("pipeline_new.bar.cancel"),
  )
  .icon(Icon::X)
  .pinned();
  let (create, cancelling, hint): (Element, Element, Reason) = match creating {
    Creating::Off(why) => {
      let words: Arc<str> = why.text().into();
      (
        Tooltip::new("pipeline-new-create-why", words.clone()).wrap(ctx, create.unavailable(words)),
        cancelling.press(cancel).into(),
        why,
      )
    }
    Creating::Ready(press, says) => (
      create.press(press).into(),
      cancelling.press(cancel).into(),
      Reason::plain(says.to_string()),
    ),
    Creating::Busy(says) => (
      create.loading().into(),
      cancelling.disabled().into(),
      Reason::plain(says.to_string()),
    ),
  };
  if !hint.names_steps() {
    return bar_saying(vec![create, cancelling], Some((hint.text(), PaletteColor::TextMuted)));
  }
  bar_ending(
    "team-hub-actions",
    vec![create, cancelling],
    None,
    Some(linked(&hint, opens)),
  )
}

/// A reason that names steps, each of them a link that opens it in the panel.
fn linked(reason: &Reason, opens: &dyn Fn(&StepKey) -> Press) -> Element {
  Row::new()
    .id("team-hub-actions-hint")
    .min_width(Dimension::Px(0.0))
    .align_items(Alignment::Center)
    .clip()
    .with_children(reason.pieces.iter().map(|piece| -> Element {
      match piece {
        Piece::Words(words) => Text::new(words).variant(HubText::PanelHint.role()).nowrap().into(),
        Piece::Step(key, words) => Focus::Link
          .apply(opens(key).button())
          .id(format!("pipeline-new-reason-{key}"))
          .describe("a11y_name", words.clone())
          .child(Text::new(words).variant(HubText::PanelLink.role()).nowrap())
          .into(),
      }
    }))
    .into()
}

/// What a refused create says over the bar: why, and that nothing was stored.
pub(super) fn refused(ctx: &Ctx, name: &str, reason: &Message) -> Element {
  let notice = Notice::new(
    "pipeline-new-refused",
    NoticeTone::Danger,
    ctx.t_args(
      "pipeline_new.bar.refusedTitle",
      [
        ("name", name.to_owned()),
        ("reason", message_text(ctx, reason).to_string()),
      ],
    ),
  )
  .icon(Icon::CircleX)
  .body(ctx.t("pipeline_new.bar.refusedBody"));
  Column::new()
    .id("pipeline-new-bar-notice")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Sections.role(), SpacingSize::Sm))
    .child(notice.title_style(HubText::NoticeTitle.role()).flat())
    .into()
}
