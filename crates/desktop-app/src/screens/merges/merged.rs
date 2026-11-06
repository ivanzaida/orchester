//! The merged runs (`Section · Today` `X3GLf` in 09 `Os7b3`), newest first, a section per local day: *Today*,
//! *Yesterday*, then the weekday and the date. A row has the `success` git-merge glyph, how the run landed in
//! `text_secondary`, who asked and when, and no action; its chevron opens the run's attempts. Older days are read as
//! the list scrolls (`super`).

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};
use orchester_workspace::tasks::merge_attempts::merges::MergedRunsDay;

use super::{
  attempt_page,
  row::{self, Shown},
  words,
};
use crate::{project::ShownProject, ui::icon::Icon};

/// A section per day of `days`, `merges-day-0` the newest.
pub(super) fn days(ctx: &mut Ctx, project: &ShownProject, days: &[MergedRunsDay]) -> Vec<Element> {
  days
    .iter()
    .enumerate()
    .map(|(index, day)| {
      let rows = day
        .runs
        .iter()
        .enumerate()
        .map(|(at, run)| {
          let id = format!("merges-merged-{}", run.run.run);
          let line = row::line(&id, &words::merged_line(ctx, run), PaletteColor::TextSecondary);
          let shown = Shown {
            glyph: (Icon::GitMerge, PaletteColor::Success),
            line: (line, "merged"),
            meta: words::merged_meta(ctx, run),
            action: None,
            opens: attempt_page(ctx, project, &run.run),
            id,
          };
          row::row(ctx, &run.run, shown, at + 1 == day.runs.len())
        })
        .collect();
      let id = format!("merges-day-{index}");
      row::section(&id, &words::day(ctx, day.day), rows)
    })
    .collect()
}
