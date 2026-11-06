//! The top of the attempt page (09b `PVZKL`): the breadcrumb *← Review / Merges / KON-12 · run #8* (`A99gLf`), the
//! task's title with the page's actions (`KOUkd`: a ghost action, *Open task*, and the primary action of how the run
//! stands), and the status line (`E8U3x`): a 14 glyph and the status in 13/500 in `warning`, `info` or `success`, then
//! the run, its attempts and the last one's time and asker in 12 `text_muted`. While Orchester waits to try again, the
//! status counts down in a component of its own, so a second's tick renders nothing else.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  integration::{attempt::MergeAsker, retry::RETRIES},
  time::Timestamp,
};
use orchester_workspace::tasks::merge_attempts::page::AttemptPage;

use super::{
  standing::Standing,
  words::{asked, attempt_time},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    icon::{Icon, icon},
  },
};

const TICK: Duration = Duration::from_secs(1);

/// *← Review / Merges / KON-12 · run #8*: Review opens the hub on its Merge queue, Merges the Merges tab.
pub(in crate::screens::merges) fn crumbs(ctx: &mut Ctx, project: &ShownProject, (key, run): (&str, u32)) -> Element {
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let trail = vec![
    Crumb::to(
      ctx.t("shell.screens.review"),
      navigation.to(Route::Project(id, ProjectPage::MergeQueue)),
    ),
    Crumb::to(
      ctx.t("shell.screens.merges"),
      navigation.to(Route::Project(id, ProjectPage::Merges)),
    ),
    Crumb::here(ctx.t_args("merges.page.crumb", [("key", key.to_owned()), ("run", run.to_string())])).mono(),
  ];
  Row::new()
    .id("merge-attempt-crumbs-row")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted))
    .child(breadcrumb(ctx, "merge-attempt-crumbs", CrumbSeparator::Slash, trail))
    .into()
}

/// The title and, at the end of its row, `actions`: the ghost one, *Open task*, the primary one.
pub(crate) fn title(title: &str, actions: Vec<Element>) -> Element {
  title_row(
    Text::new(title)
      .id("merge-attempt-title")
      .variant(TypographyStyle::Heading)
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .nowrap(),
    actions,
  )
}

/// `title`, which takes the room left, and at the end of its row `actions`; the title is a bone while it is read.
pub(crate) fn title_row(title: impl Into<Element>, actions: Vec<Element>) -> Element {
  Row::new()
    .id("merge-attempt-header")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(title)
    .with_children(actions)
    .into()
}

/// The status line of `page`, whose run stands as `standing`; `watch`, what looks at Orchester's wait to try again
/// while the run merges, renders nothing at its end.
pub(super) fn status(
  ctx: &mut Ctx,
  page: &AttemptPage,
  (standing, base): (Standing, &str),
  watch: Option<Element>,
) -> Element {
  let (glyph, color) = standing.glyph();
  let meta = meta(ctx, page, standing, base);
  let words: Element = match standing {
    Standing::Retrying(retrying) if retrying.next.is_some() => ctx.mount_keyed::<Countdown>(
      "merge-attempt-countdown",
      Next {
        last: page.last().map_or(MergeAsker::Person, |last| last.attempt.asked_by()),
        at: retrying.next,
      },
    ),
    _ => status_text(&standing.status(ctx, base), color.clone()),
  };
  Row::new()
    .id("merge-attempt-status")
    .describe("standing", standing.name())
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Md, color))
    .child(words)
    .child(
      Text::new(&meta)
        .id("merge-attempt-status-meta")
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .with_children(watch)
    .into()
}

fn status_text(text: &str, color: PaletteColor) -> Element {
  Text::new(text)
    .id("merge-attempt-status-text")
    .variant(ExtraTypography::Strong.role())
    .color(color)
    .nowrap()
    .into()
}

/// *run #8 · 2 attempts · last at 12:41, by you from the task page*; a merged run names its commit, a run Orchester
/// tries again that nothing was merged, one stopped while bringing the merge in that its base branch did not move.
fn meta(ctx: &Ctx, page: &AttemptPage, standing: Standing, base: &str) -> Arc<str> {
  let mut parts = vec![ctx.t_args("merges.page.meta.run", [("run", page.run.number.get().to_string())])];
  if let Standing::Retrying(_) = standing {
    parts.push(ctx.t("merges.page.meta.nothingMerged"));
    return super::super::words::joined(ctx, &parts);
  }
  match (standing, &page.merge) {
    (Standing::Merged, Some(merge)) => parts.push(super::words::made(ctx, &merge.revision, merge.kind)),
    _ if page.attempts.len() == 1 => parts.push(ctx.t("merges.page.meta.attemptsOne")),
    _ => parts.push(ctx.t_args(
      "merges.page.meta.attemptsOther",
      [("count", page.attempts.len().to_string())],
    )),
  }
  if let Some(last) = page.last() {
    let time = attempt_time(ctx, last.attempt.at(), false).to_string();
    let by = asked(ctx, &last.attempt).to_string();
    let key = if page.attempts.len() > 1 && standing != Standing::Merged {
      "merges.page.meta.lastAt"
    } else {
      "merges.page.meta.at"
    };
    parts.push(ctx.t_args(key, [("time", time), ("by", by)]));
  }
  if standing == Standing::LeftChanged {
    parts.push(ctx.t_args("merges.page.meta.didNotMove", [("branch", base)]));
  }
  super::super::words::joined(ctx, &parts)
}

#[derive(Clone, Debug, PartialEq)]
struct Next {
  /// Who asked for the last try.
  last: MergeAsker,
  /// When Orchester tries next.
  at: Option<Timestamp>,
}

impl DevtoolsInspectable for Next {}

/// *Retrying by itself · attempt 3 of 3 in 18 s*, counting down a second at a time.
struct Countdown {
  tick: Signal<u64>,
  _interval: Interval,
}

impl Component for Countdown {
  type Props = Next;

  fn create(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    let interval = ctx.create_interval(TICK, move || next.update(|tick| *tick += 1));
    interval.start();
    Self {
      tick,
      _interval: interval,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Next { last, at } = ctx.props::<Next>().clone();
    // Read so every tick renders the line again.
    self.tick.get();
    let attempt = match last {
      MergeAsker::Retry { number } => number.get().saturating_add(1).min(RETRIES),
      _ => 1,
    };
    let seconds = at.map_or(0, |at| {
      ((at.unix_millis() - Timestamp::now().unix_millis()).max(0) + 999) / 1000
    });
    let (attempt, of) = (attempt.to_string(), RETRIES.to_string());
    let text = if seconds == 0 {
      ctx.t_args("merges.page.status.retryingNow", [("attempt", attempt), ("of", of)])
    } else {
      ctx.t_args(
        "merges.page.status.retrying",
        [("attempt", attempt), ("of", of), ("seconds", seconds.to_string())],
      )
    };
    Column::new().child(status_text(&text, PaletteColor::Info))
  }
}
