//! A run that was merged after the attempt the page opens on (the merge queue, a later attempt, the Orchestrator, or a
//! person outside Orchester; ORC-917): the page says so first, in the strip under the status line in `success`, with
//! the branch, the revision, and when and how, and shows the attempt as history, with no action to fix what no longer
//! needs fixing. There is no 13-series state for it: it is 13e's *What happened* strip over the usual Checks and
//! Attempts, with *The last attempt* between them. Orchester recorded no time of a merge made outside it, and the strip
//! says so rather than guess; the revision it shows is the run's own head, which the branch holds.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::integration::{
  MergeKind,
  attempt::{AttemptOutcome, RecordedAttempt},
};
use orchester_workspace::{display::short_revision, tasks::merge_attempts::page::AttemptPage};

use super::{
  checks, conflicts,
  lines::{attempt_line, joined, when},
  section, section_head,
  standing::Standing,
  strip::strip,
  words::attempt_time,
};
use crate::{theme::AttemptText, ui::icon::Icon};

/// The last attempt of `page`, whose run stands as `standing`, when the run was merged after it; `None` when the
/// attempt itself landed or the run is not merged.
pub(super) fn later(page: &AttemptPage, standing: Standing) -> Option<&RecordedAttempt> {
  let last = page.last()?;
  (standing.is_merged() && !last.attempt.outcome().landed()).then_some(last)
}

/// *Merged into main at d08e094*, and under it how and when; for a merge made outside Orchester, that it was.
pub(super) fn banner(ctx: &Ctx, page: &AttemptPage) -> Element {
  let (title, body) = match (&page.merge, page.delivery.merged_into()) {
    (Some(merge), _) => {
      let how = ctx.t(match merge.kind {
        MergeKind::MergeCommit => "merges.page.banner.mergeCommit",
        MergeKind::Squash => "merges.page.banner.squash",
        MergeKind::FastForward | MergeKind::AlreadyMerged => "merges.page.banner.fastForward",
      });
      (
        ctx.t_args(
          "merges.page.banner.merged",
          [
            ("branch", merge.into.as_str()),
            ("revision", short_revision(merge.revision.as_str())),
          ],
        ),
        joined(ctx, &[how, when(ctx, merge.merged_at)]),
      )
    }
    (None, into) => {
      let branch = into.unwrap_or_default();
      // The run's own head is in the branch; only the time of the merge is unknown.
      let body = match &page.reviewed {
        Some(head) => ctx.t_args(
          "merges.page.banner.outsideHead",
          [("branch", branch), ("revision", short_revision(head.as_str()))],
        ),
        None => ctx.t_args("merges.page.banner.outsideBody", [("branch", branch)]),
      };
      (ctx.t_args("merges.page.banner.outside", [("branch", branch)]), body)
    }
  };
  strip(
    ("merge-attempt-merged-banner", "merge-attempt-merged-banner-words"),
    (Icon::CircleCheck, PaletteColor::Success),
    (&title, &body),
  )
}

/// The attempt as history: how it ended and that the run was merged later, and the files it conflicted on, which no
/// longer wait to be resolved.
pub(super) fn history(ctx: &Ctx, page: &AttemptPage, last: &RecordedAttempt, base: &str) -> Vec<Element> {
  let seconds = super::attempts::to_the_second(&page.attempts);
  let count = checks::heading_count(ctx, (last.sequence, attempt_time(ctx, last.attempt.at(), seconds)));
  let words = |id: &str, text: &str| -> Element {
    Text::new(text)
      .id(id.to_owned())
      .variant(AttemptText::Explanation.role())
      .width(Dimension::Pct(100.0))
      .into()
  };
  let mut sections = vec![section(
    "merge-attempt-history",
    section_head(
      "merge-attempt-history-head",
      &ctx.t("merges.page.history"),
      (Some(&count), None),
    ),
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(words(
        "merge-attempt-history-outcome",
        &attempt_line(ctx, &last.attempt, base),
      ))
      .child(words("merge-attempt-history-note", &ctx.t("merges.page.historyNote")))
      .into(),
  )];
  if let AttemptOutcome::Conflicts { files } = last.attempt.outcome() {
    let then = ctx.t("merges.page.conflictedThen");
    sections.push(section(
      "merge-attempt-history-conflicts",
      section_head(
        "merge-attempt-conflicting-head",
        &ctx.t("merges.page.conflicting"),
        (Some(&files.len().to_string()), None),
      ),
      conflicts::files_list(
        files
          .iter()
          .map(|file| (file.as_str(), then.clone(), PaletteColor::TextMuted)),
      ),
    ));
  }
  sections
}
