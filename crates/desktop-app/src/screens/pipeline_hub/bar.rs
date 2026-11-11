//! The pinned bar of the panel of a pipeline (`Actions` `uat7n` in `Pipelines · v2` P1): *Save* and *Cancel*, always
//! there and in the same place from the first frame, and at the end what *Save* makes: the next version, which runs
//! already started do not take (*Save makes v4 · runs keep the version they started on*); for a pipeline that was never
//! published and can be, *Save publishes it as v1*; for one that cannot be published yet, that *Save* keeps it as a
//! draft, though a pipeline holds no draft any more (D-032): such a save is refused and says why. After *Restore* the
//! bar says which version the unsaved changes are. The bar is the Agents hub's ([`save_bar`]). Above it, in place, what
//! a refused save says, and the version a save made.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::Column,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::VersionNumber;
use orchester_workspace::{
  messages::Message,
  pipelines::local_edit::{Unpublishable, version_limit},
};

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

/// What the bar says at its end, from what the panel holds.
pub(super) struct Line<'a> {
  pub entry: &'a Entry,
  /// The edit has changes that are not saved, or text the check refuses.
  pub unsaved: bool,
  /// What to correct before saving: text of a step the check refuses.
  pub fix: Option<String>,
  /// What keeps what the panel shows from being published; `None` when it can be.
  pub unpublishable: Option<Unpublishable>,
  /// The version *Restore* put in the edit, while its changes are not saved.
  pub restored: Option<VersionNumber>,
}

/// The bar: *Save*, *Cancel* (`None` while there is nothing to drop, or a save is under way) and `line`'s words.
pub(super) fn bar(ctx: &mut Ctx, (save, cancel): (Save, Option<Press>), line: &Line<'_>) -> Element {
  let hint = hint(ctx, line);
  save_bar::bar(ctx, "pipeline-hub", (save, cancel), hint)
}

/// What publishing what the panel shows as it stands would refuse, in its own words: the pipeline has its last
/// version number, or a step publishing does not take; `None` for the steps' own problems, which *Ready?* names.
fn refused(ctx: &Ctx, line: &Line<'_>) -> Option<String> {
  if let Some(limit) = version_limit(line.entry.number) {
    return Some(message_text(ctx, &limit).to_string());
  }
  match line.unpublishable.as_ref()? {
    Unpublishable::Refused(why) => Some(message_text(ctx, why).to_string()),
    Unpublishable::NoSteps | Unpublishable::Steps | Unpublishable::Broken => None,
  }
}

/// The words at the bar's end: what to correct, in `warning`, while there is something; else what *Save* makes, after
/// *Unsaved changes ·* (or *Restored v2 · not saved ·*) while the edit has changes, so it shows where *Save* is
/// pressed.
fn hint(ctx: &Ctx, line: &Line<'_>) -> (String, PaletteColor) {
  if let Some(fix) = line.fix.clone() {
    return (fix, PaletteColor::Warning);
  }
  let (makes, color) = makes(ctx, line);
  if !line.unsaved {
    return (makes, color);
  }
  let said = match line.restored {
    Some(version) => ctx.t_args(
      "pipeline_hub.bar.restored",
      [("version", version.to_string()), ("rest", makes)],
    ),
    None => ctx.t_args("pipeline_hub.line.unsaved", [("rest", makes)]),
  };
  (said.to_string(), color)
}

/// What *Save* makes: the next version; the first, for a pipeline never published; only the draft, while what the
/// panel shows cannot be published, with why when publishing would refuse it ([`refused`]). A published pipeline that
/// holds no change and could not be published again says only that nothing changed: what keeps its version from running
/// is *Ready?*'s to say.
fn makes(ctx: &Ctx, line: &Line<'_>) -> (String, PaletteColor) {
  let blocked = line.unpublishable.is_some() || version_limit(line.entry.number).is_some();
  if let Some(number) = line.entry.number.filter(|_| !line.unsaved && blocked) {
    let said = ctx.t_args("pipeline_hub.bar.noChanges", [("version", number.to_string())]);
    return (said.to_string(), PaletteColor::TextMuted);
  }
  if let Some(why) = refused(ctx, line) {
    let said = ctx.t_args("pipeline_hub.bar.keepsDraftRefused", [("why", why)]);
    return (said.to_string(), PaletteColor::TextMuted);
  }
  let next = line
    .entry
    .number
    .map_or(Some(VersionNumber::FIRST), VersionNumber::next)
    .map_or_else(String::new, |next| next.to_string());
  let words = match (line.unpublishable.is_some(), line.entry.number) {
    (true, _) => ctx.t_args("pipeline_hub.bar.keepsDraft", [("version", next)]),
    (false, None) => ctx.t_args("pipeline_hub.bar.publishesFirst", [("version", next)]),
    (false, Some(_)) => return save_bar::makes(ctx, line.entry.number),
  };
  (words.to_string(), PaletteColor::TextMuted)
}

/// What the last save said, over the bar: a refusal (*Couldn't save …*, nothing stored and everything typed kept), or
/// the version a save made and whether the next run takes it.
pub(super) enum Said<'a> {
  Refused(&'a Message),
  /// The version, and whether what was saved can run.
  Made(VersionNumber, bool),
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
    Said::Made(version, runs) => Notice::new(
      "pipeline-hub-saved",
      NoticeTone::Success,
      ctx.t_args(
        if *runs {
          "pipeline_hub.bar.savedTitle"
        } else {
          "pipeline_hub.bar.savedTitleWaits"
        },
        [("version", version.to_string())],
      ),
    )
    .icon(Icon::CircleCheck),
  };
  Column::new()
    .id("pipeline-hub-bar-notice")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Sections.role(), SpacingSize::Sm))
    .child(notice.title_style(HubText::NoticeTitle.role()).flat())
    .into()
}
