//! The pinned bar of the panel of a pipeline (`Actions` `uat7n` in `Pipelines · v2` P1): *Save* and *Cancel*, always
//! there and in the same place from the first frame, and at the end what *Save* makes: the next version, which runs
//! already started do not take (*Save makes v4 · runs keep the version they started on*); for a pipeline that was never
//! published its first, which New task offers once every step is ready. The bar is the Agents hub's
//! ([`save_bar`]). Above it, in place, what a refused save says and what a save that kept the draft says.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::VersionNumber;
use orchester_workspace::messages::Message;

use super::model::Entry;
use crate::{
  screens::local_hub::save_bar::{self, Save},
  theme::{HubSpacing, HubText},
  ui::{
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    pressable::Press,
  },
};

/// The bar for `entry`: *Save*, *Cancel* (`None` while nothing differs, or a save is under way) and what *Save* makes,
/// after *Unsaved changes ·* while the edit has changes, so it shows where *Save* is pressed.
pub(super) fn bar(ctx: &Ctx, (save, cancel): (Save, Option<Press>), (entry, changed): (&Entry, bool)) -> Element {
  let hint = match entry.number {
    Some(_) => save_bar::makes(ctx, entry.number),
    None => (
      ctx
        .t_args(
          "pipeline_hub.bar.makesFirst",
          [("version", VersionNumber::FIRST.to_string())],
        )
        .to_string(),
      lurq::app::theme::PaletteColor::TextMuted,
    ),
  };
  let hint = if changed {
    (
      ctx
        .t_args("pipeline_hub.line.unsaved", [("rest", hint.0)])
        .to_string(),
      hint.1,
    )
  } else {
    hint
  };
  save_bar::bar(ctx, "pipeline-hub", (save, cancel), hint)
}

/// What the last save said, over the bar: a refusal (*Couldn't save …*, nothing stored and everything typed kept), the
/// draft a save that could not publish kept and why, or the version a save made.
pub(super) enum Said<'a> {
  Refused(&'a Message),
  Kept(&'a Message),
  Made(VersionNumber),
}

/// The notice for `said` about `entry`, with its padding.
pub(super) fn notice(ctx: &Ctx, entry: &Entry, said: &Said<'_>) -> Element {
  let notice = match said {
    Said::Refused(reason) => Notice::new(
      "pipeline-hub-refused",
      NoticeTone::Danger,
      ctx.t_args(
        "pipeline_hub.bar.refusedTitle",
        [
          ("name", entry.name.to_string()),
          ("reason", message_text(ctx, reason).to_string()),
        ],
      ),
    )
    .icon(Icon::CircleX)
    .body(ctx.t("pipeline_hub.bar.refusedBody")),
    Said::Made(version) => Notice::new(
      "pipeline-hub-saved",
      NoticeTone::Success,
      ctx.t_args("pipeline_hub.bar.savedTitle", [("version", version.to_string())]),
    )
    .icon(Icon::CircleCheck),
    Said::Kept(why) => Notice::new(
      "pipeline-hub-kept",
      NoticeTone::Warning,
      ctx.t_args("pipeline_hub.bar.keptTitle", [("name", entry.name.to_string())]),
    )
    .icon(Icon::FilePen)
    .body(message_text(ctx, why)),
  };
  Column::new()
    .id("pipeline-hub-bar-notice")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Sections.role(), SpacingSize::Sm))
    .child(notice.title_style(HubText::NoticeTitle.role()).flat())
    .into()
}
