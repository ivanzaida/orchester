//! The pinned bar of a new pipeline's panel (`Actions` `vtZ9C` in P14 `Rszkt`): *Create v1* and *Cancel*, and at its end
//! why *Create v1* is off (the reason also in its tooltip, as *Save*'s is) or what it saves. Above it, in place, what a
//! refused create says: nothing was stored and everything typed stays.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::Column,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::messages::Message;

use crate::{
  screens::team_hub::bar::bar_saying,
  theme::{HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    pressable::Press,
    tooltip::Tooltip,
  },
};

/// What *Create v1* can do now.
pub(super) enum Creating {
  /// It cannot create the pipeline as it stands, and why.
  Off(Arc<str>),
  /// It creates it, which the bar says.
  Ready(Press, Arc<str>),
  /// It is storing it, which the bar says.
  Busy(Arc<str>),
}

/// The bar: *Create v1*, *Cancel* (off while the pipeline is stored) and what *Create v1* does or waits for.
pub(super) fn bar(ctx: &mut Ctx, creating: Creating, cancel: Press) -> Element {
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
  let (create, cancelling, hint): (Element, Element, Arc<str>) = match creating {
    Creating::Off(why) => (
      Tooltip::new("pipeline-new-create-why", why.clone()).wrap(ctx, create.unavailable(why.clone())),
      cancelling.press(cancel).into(),
      why,
    ),
    Creating::Ready(press, says) => (create.press(press).into(), cancelling.press(cancel).into(), says),
    Creating::Busy(says) => (create.loading().into(), cancelling.disabled().into(), says),
  };
  bar_saying(
    vec![create, cancelling],
    Some((hint.to_string(), PaletteColor::TextMuted)),
  )
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
