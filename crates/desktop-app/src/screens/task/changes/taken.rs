//! Over the files of a run whose branch took in commits from branches other than its target
//! (`orchester_core::integration::TakenIn`): a warning that merging the run brings them too, with how many and from
//! where, and a press that shows its agent's own changes alone, or everything a merge brings again.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::integration::TakenIn;

use super::{grouped, load::ChangesReads};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  notice::{Notice, NoticeTone},
};

/// The warning over the files; `own_only` while they are the agent's own changes.
pub(super) fn notice(ctx: &Ctx, reads: &ChangesReads, (taken, own_only): (&TakenIn, bool)) -> Element {
  let commits = grouped(ctx, u32::try_from(taken.commits).unwrap_or(u32::MAX));
  let title = match &taken.from {
    Some(branch) => ctx.t_args(
      "changes.takenIn.titleFrom",
      [("commits", commits.as_str()), ("branch", branch.as_str())],
    ),
    None => ctx.t_args("changes.takenIn.title", [("commits", commits.as_str())]),
  };
  let (detail, press) = if own_only {
    ("changes.takenIn.ownShown", "changes.takenIn.showAll")
  } else {
    ("changes.takenIn.allShown", "changes.takenIn.showOwn")
  };
  let reads = reads.clone();
  let switch = ActionButton::new("changes-taken-in-switch", ButtonKind::Secondary, ctx.t(press))
    .small()
    .on_click(move || reads.show_own_only(!own_only));
  let notice = Notice::new("changes-taken-in", NoticeTone::Warning, title)
    .icon(Icon::GitMerge)
    .body(ctx.t(detail))
    .action(switch);
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Section, SpacingSize::Md))
    .child(notice)
    .into()
}
