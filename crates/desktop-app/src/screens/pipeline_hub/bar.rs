//! The pinned bar of the panel of a pipeline (`Actions` `uat7n` in `Pipelines · v2` P1): *Save* and *Cancel*, always
//! there and in the same place from the first frame, and at the end what *Save* makes: the next version, which runs
//! already started do not take (*Save makes v4 · runs keep the version they started on*). A pipeline is stored only as
//! a published version (D-032), so while the edit cannot be published *Save* is off and the bar says what to do first
//! (*Choose a profile for 02 Review to save*, P8 `vujkk`); nothing is stored. After *Restore* the bar says which
//! version the unsaved changes are. The bar is the Agents hub's ([`save_bar`]). Above it, in place, what a refused save
//! says, and the version a save made.

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
  pipelines::{
    flow::FlowRead,
    local_edit::{Unpublishable, blocking_steps, version_limit},
    ready::{Blocked, StepProblem},
  },
};

use super::{flow::number, model::Entry};
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
  /// The flow of what the panel shows, whose steps say what keeps it from being published.
  pub read: &'a FlowRead,
  /// The version *Restore* put in the edit, while its changes are not saved.
  pub restored: Option<VersionNumber>,
}

/// The bar: *Save*, *Cancel* (`None` while there is nothing to drop, or a save is under way) and `line`'s words.
pub(super) fn bar(ctx: &mut Ctx, (save, cancel): (Save, Option<Press>), line: &Line<'_>) -> Element {
  let hint = hint(ctx, line);
  save_bar::bar(ctx, "pipeline-hub", (save, cancel), hint)
}

/// Why *Save* is off for what the panel shows: what to do before it can be published, or why publishing refuses it (the
/// pipeline has its last version number, or a step publishing does not take); `None` when it can be published.
pub(super) fn save_off(ctx: &Ctx, line: &Line<'_>) -> Option<String> {
  if let Some(limit) = version_limit(line.entry.number) {
    return Some(message_text(ctx, &limit).to_string());
  }
  let words = match line.unpublishable.as_ref()? {
    Unpublishable::NoSteps => ctx.t("pipeline_hub.bar.offNoSteps"),
    Unpublishable::Steps => steps_off(ctx, &blocking_steps(line.read))?,
    Unpublishable::Broken => ctx.t("pipeline_hub.bar.offBroken"),
    Unpublishable::Refused(why) => message_text(ctx, why),
  };
  Some(words.to_string())
}

/// What to do about the steps that keep the edit from being published: the one step's (*Choose a profile for 02
/// Review to save*), or how many there are.
fn steps_off(ctx: &Ctx, steps: &[StepProblem]) -> Option<std::sync::Arc<str>> {
  let step = match steps {
    [] => return None,
    [only] => only,
    many => return Some(ctx.t_args("pipeline_hub.bar.offSteps", [("count", many.len().to_string())])),
  };
  let key = match &step.blocked {
    Blocked::NoProfile | Blocked::Gone { tool: false, .. } => "pipeline_hub.bar.offProfile",
    Blocked::NoTool | Blocked::Gone { tool: true, .. } | Blocked::ToolNotRunnable { .. } => {
      "pipeline_hub.bar.offTool"
    }
    Blocked::ProfileNotPublished { .. } | Blocked::ProfileConnectionGone { .. } => "pipeline_hub.bar.offProfileReady",
    Blocked::ToolNotPublished { .. } => "pipeline_hub.bar.offToolReady",
  };
  Some(ctx.t_args(
    key,
    [("number", number(step.number)), ("step", step.name.to_string())],
  ))
}

/// The words at the bar's end: what to correct, in `warning`, while there is something; while the edit has changes,
/// why *Save* is off ([`save_off`]) or what it makes after *Unsaved changes ·* (or *Restored v2 · not saved ·*), so it
/// shows where *Save* is pressed; else what *Save* would make.
fn hint(ctx: &Ctx, line: &Line<'_>) -> (String, PaletteColor) {
  if let Some(fix) = line.fix.clone() {
    return (fix, PaletteColor::Warning);
  }
  if !line.unsaved {
    return makes(ctx, line);
  }
  let (rest, off) = save_off(ctx, line).map_or_else(|| (makes(ctx, line).0, false), |why| (why, true));
  let said = match (line.restored, off) {
    (Some(version), _) => ctx.t_args(
      "pipeline_hub.bar.restored",
      [("version", version.to_string()), ("rest", rest)],
    ),
    (None, true) => return (rest, PaletteColor::TextMuted),
    (None, false) => ctx.t_args("pipeline_hub.line.unsaved", [("rest", rest)]),
  };
  (said.to_string(), PaletteColor::TextMuted)
}

/// What *Save* makes: the next version. A pipeline that holds no change and could not be published again says only
/// that nothing changed: what keeps its version from running is *Ready?*'s to say.
fn makes(ctx: &Ctx, line: &Line<'_>) -> (String, PaletteColor) {
  let blocked = line.unpublishable.is_some() || version_limit(line.entry.number).is_some();
  if let Some(number) = line.entry.number.filter(|_| !line.unsaved && blocked) {
    let said = ctx.t_args("pipeline_hub.bar.noChanges", [("version", number.to_string())]);
    return (said.to_string(), PaletteColor::TextMuted);
  }
  save_bar::makes(ctx, line.entry.number)
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
