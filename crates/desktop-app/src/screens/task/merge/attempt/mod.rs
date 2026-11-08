//! A run's merge attempts (ORC-363, ORC-915; `Review - v2` 13a `i0K3xB` … 13i `yHy6H`, row 7 `wbM48`), under its task
//! at `/project/<id>/tasks/<KEY>/merge/<run>`: the breadcrumb, the task's title, which wraps beside *Copy details*, the
//! status line, *What stopped it* (*What happened* once merged) in the words of the stage that ended the last attempt,
//! the run's facts with *Runner log folder*; then, scrolling, the conflicting files or the files left changed, the last
//! attempt's *Checks*, and *Attempts*, newest first with Orchester's retries, each expanding into its own checks; and
//! the bar, which holds the one action that moves the run on. The page is read from the run's attempts
//! (`merge_attempts::page`) again whenever the project's tasks or runs change, the merge queue moves, a merge job ends,
//! or a button here changed something; the attempts last read stand in while it reads them anew, and while it reads
//! them for the first time the page is bones ([`reading`]).

pub(crate) mod actions;
mod attempts;
mod bar;
mod checks;
mod conflicts;
mod details;
mod facts;
pub(crate) mod head;
mod left;
pub(crate) mod lines;
mod merged;
pub(crate) mod read;
mod reading;
mod retry;
pub(crate) mod standing;
mod strip;
pub(super) mod watch;
mod words;

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::{Alignment, layout_kind::ScrollState},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{integration::attempt::AttemptOutcome, project::TaskKey, run::RunNumber};
use orchester_workspace::{
  messages::Message,
  tasks::{integration_jobs::JobState, merge_attempts::page::AttemptPage},
};

use self::{
  actions::{Actions, Busy},
  attempts::Expanded,
  checks::{Branches, Folds},
  conflicts::{AskAction, Conflicts, Resolving, Wanted},
  read::{Read, read},
  standing::Standing,
};
use crate::{
  navigation::{Navigation, link::Open},
  project::ShownProject,
  routes::{ProjectPage, Route},
  screens::review::task::agent_field::AgentChoice,
  services::Services,
  theme::{AttemptSpacing, DeliverySpacing, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    code_box::Copier,
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    state::{EmptyState, load_error},
  },
};

/// How long *Copy details* says it copied before it offers to copy again.
const COPY_FEEDBACK: Duration = Duration::from_secs(2);
/// The branch a line names when the run did not record the one it merges into.
const NO_BRANCH: &str = "—";
/// An empty state's card (`jhPCL`).
const EMPTY_WIDTH: f32 = 440.0;

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
  key: TaskKey,
  number: RunNumber,
}

impl DevtoolsInspectable for Props {}

/// The attempt page of run `number` of the task `key` in `project`.
pub(crate) fn screen(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey, number: RunNumber) -> Element {
  ctx.mount_keyed::<AttemptScreen>(
    &format!("merge-attempt-{}-{key}-{}", project.open.project.id(), number.get()),
    Props {
      project: project.clone(),
      key: key.clone(),
      number,
    },
  )
}

/// What *Copy details* says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Copied {
  #[default]
  Not,
  Done,
  Failed,
}

impl DevtoolsInspectable for Copied {}

struct AttemptScreen {
  /// Counts Try again after a failed read, and the changes this page's buttons made.
  again: Signal<u64>,
  failed: Signal<Option<Message>>,
  busy: Signal<Busy>,
  expanded: Signal<Expanded>,
  /// The checks that passed first are unfolded.
  shown: Signal<bool>,
  /// The agent *Ask the agent to resolve* returns the run to, chosen for this time.
  choice: AgentChoice,
  person_only: Signal<bool>,
  copied: Signal<Copied>,
  reset: Timeout,
  scroll: ScrollState,
}

impl Component for AttemptScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let copied = ctx.signal(Copied::Not);
    let back = copied.clone();
    let reset = ctx.create_timeout(COPY_FEEDBACK, move || back.set(Copied::Not));
    Self {
      again: ctx.signal(0),
      failed: ctx.signal(None),
      busy: ctx.signal(Busy::Idle),
      expanded: ctx.signal(Expanded::default()),
      shown: ctx.signal(false),
      choice: AgentChoice::new(ctx),
      person_only: ctx.signal(false),
      copied,
      reset,
      scroll: ScrollState::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, key, number } = ctx.props::<Props>().clone();
    let read = read(ctx, &project, (&key, number), self.again.get());
    let page = match &read {
      Read::Ready(page) => Some(page.as_ref()),
      _ => None,
    };
    let standing = page.and_then(Standing::of);
    let wanted = page
      .filter(|_| standing == Some(Standing::Conflicts))
      .map(|page| Wanted {
        task: page.run.task,
        run: page.run.run,
        key: page.run.key.clone(),
        number: page.run.number,
      });
    let feed = &project.feed;
    let resolving = conflicts::read(
      ctx,
      &project,
      wanted,
      (feed.saves.get(), feed.states.get(), self.again.get()),
    );
    let ask = conflicts::ask_action(ctx, &project);
    let page = match read {
      Read::Reading => return reading::page(ctx, &project, (&key, number)),
      Read::Failed(reason) => {
        let again = self.again.clone();
        let body = ctx.t_args("merges.page.readFailedBody", [("reason", message_text(ctx, &reason))]);
        return frame(ctx, &project, (&key, number))
          .child(load_error(
            ctx,
            "merge-attempt-load-error",
            (&ctx.t("merges.page.readFailed"), &body),
            move || again.update(|count| *count += 1),
          ))
          .into();
      }
      Read::Missing => {
        let body = ctx.t_args(
          "merges.page.notFoundBody",
          [("key", key.to_string()), ("run", number.get().to_string())],
        );
        return frame(ctx, &project, (&key, number))
          .child(empty(
            "merge-attempt-missing",
            Icon::SearchX,
            (ctx.t("merges.page.notFound"), body),
          ))
          .into();
      }
      Read::Ready(page) => page,
    };
    let shown = Shown {
      project: &project,
      page: &page,
      resolving: resolving.as_ref(),
      ask: &ask,
    };
    frame(ctx, &project, (&key, number))
      .with_children(self.body(ctx, &shown, standing))
      .into()
  }
}

/// The page's frame with its breadcrumb: 20 over it, 24 at its sides, 16 under it, its parts 12 apart.
pub(super) fn frame(ctx: &mut Ctx, project: &ShownProject, (key, number): (&TaskKey, RunNumber)) -> Column {
  Column::new()
    .id("screen-merge-attempt")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(AttemptSpacing::Top.role())
        .left(SpacingSize::Xl)
        .right(SpacingSize::Xl)
        .bottom(SpacingSize::Lg),
    )
    .spacing(SpacingSize::Md)
    .child(head::crumbs(ctx, project, (key, number.get())))
}

/// What the page shows and reads beside it.
struct Shown<'a> {
  project: &'a ShownProject,
  page: &'a AttemptPage,
  resolving: Option<&'a Resolving>,
  ask: &'a AskAction,
}

impl Shown<'_> {
  fn base(&self) -> &str {
    self.page.run.base.as_ref().map_or(NO_BRANCH, |branch| branch.as_str())
  }

  /// The review of the run's task, where the conflicts are marked.
  fn review(&self, ctx: &mut Ctx) -> Open {
    Navigation::of(ctx).to(Route::Project(
      self.project.open.project.id(),
      ProjectPage::TaskReview(self.page.run.key.clone()),
    ))
  }
}

impl AttemptScreen {
  /// The header, the status line, the strip, the facts, the body and the bar, as `standing` tells them; with no attempt
  /// yet, the header and an empty state.
  fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>, standing: Option<Standing>) -> Vec<Element> {
    let page = shown.page;
    let Some(standing) = standing else {
      let body = ctx.t_args(
        "merges.page.noAttemptsBody",
        [("run", page.run.number.get().to_string())],
      );
      return vec![
        head::wrapping_title(head::heading(&page.run.title), Vec::new()),
        empty(
          "merge-attempt-none",
          Icon::GitMerge,
          (ctx.t("merges.page.noAttempts"), body),
        ),
      ];
    };
    let actions = Actions::new(
      (
        Services::of(ctx).worktree_remover(&shown.project.open),
        Services::of(ctx).programs,
        page.run.run,
      ),
      ctx.t("task.listSeparator"),
      (self.failed.clone(), self.busy.clone(), self.again.clone()),
    );
    let merging = shown.project.feed.job_state.get().run(page.run.run).merge == JobState::Running;
    let watch = merging.then(|| watch::watch(ctx, page.run.run, &self.again));
    let conflicts = self.conflicts(shown, page);
    let bar = self.bar(ctx, shown, standing, (&actions, conflicts.as_ref()));
    let what = if standing.is_merged() {
      "merges.page.whatHappened"
    } else {
      "merges.page.whatStopped"
    };
    let later = merged::later(page, standing);
    let strip = if later.is_some() {
      merged::banner(ctx, page)
    } else {
      strip::strip(
        ("merge-attempt-what", "merge-attempt-explanation"),
        standing.strip_glyph(),
        (&ctx.t(what), &standing::explanation(ctx, page, standing, shown.base())),
      )
    };
    let sections = self.sections(ctx, shown, standing, conflicts.as_ref());
    let mut elements = vec![
      head::wrapping_title(
        head::heading(&page.run.title),
        vec![self.copy_details(ctx, shown, standing)],
      ),
      head::status(ctx, page, (standing, shown.base()), watch),
      strip,
      facts::facts(ctx, page, &shown.project.open.repo_root),
      self.scrolling(sections),
    ];
    elements.extend(bar);
    elements
  }

  /// What the conflicts section and the bar ask of the page, while the last attempt found conflicts.
  fn conflicts<'a>(&'a self, shown: &'a Shown<'a>, page: &'a AttemptPage) -> Option<Conflicts<'a>> {
    let Some(AttemptOutcome::Conflicts { files }) = page.last().map(|last| last.attempt.outcome()) else {
      return None;
    };
    Some(Conflicts {
      project: shown.project,
      run: page.run.run,
      files,
      resolving: shown.resolving,
      choice: &self.choice,
      ask: shown.ask,
      person_only: &self.person_only,
    })
  }

  /// *Copy details*, at the end of the header's row: what the page says of the run, to paste in an issue.
  fn copy_details(&self, ctx: &mut Ctx, shown: &Shown<'_>, standing: Standing) -> Element {
    let (glyph, key) = match self.copied.get() {
      Copied::Not => (Icon::Copy, "merges.page.copyDetails"),
      Copied::Done => (Icon::Check, "merges.page.copied"),
      Copied::Failed => (Icon::CircleAlert, "merges.page.copyFailed"),
    };
    let text = details::details(ctx, shown.page, &standing.status(ctx, shown.base()));
    let (copier, copied, reset) = (Copier::of(ctx), self.copied.clone(), self.reset.clone());
    ActionButton::new("merge-attempt-copy-details", ButtonKind::Ghost, ctx.t(key))
      .icon(glyph)
      .on_click(move || {
        copied.set(if copier.copy(&text) {
          Copied::Done
        } else {
          Copied::Failed
        });
        reset.restart();
      })
      .into()
  }

  /// The sections `sections` in the body, which scrolls.
  fn scrolling(&self, sections: Vec<Element>) -> Element {
    ScrollVertical::new(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().top(SpacingSize::Xs))
        .spacing(SpacingSize::Lg)
        .with_children(sections),
    )
    .with_scroll_state(self.scroll.clone())
    .id("merge-attempt-main")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .into()
  }

  /// The body's sections: what a page's buttons could not do, the files the state names, the last attempt's checks
  /// and the attempts.
  fn sections(
    &self,
    ctx: &mut Ctx,
    shown: &Shown<'_>,
    standing: Standing,
    conflicts: Option<&Conflicts<'_>>,
  ) -> Vec<Element> {
    let page = shown.page;
    let mut sections: Vec<Element> = Vec::new();
    let failure = self.failed.get().or_else(|| conflicts.and_then(Conflicts::failure));
    sections.extend(failure.map(|reason| {
      Notice::new(
        "merge-attempt-not-done",
        NoticeTone::Danger,
        ctx.t_args("merges.notDone", [("reason", message_text(ctx, &reason))]),
      )
      .into()
    }));
    if let (Standing::Conflicts, Some(conflicts)) = (standing, conflicts) {
      let review = shown.review(ctx);
      sections.push(conflicts.section(ctx, review));
    }
    if let Some(last) = merged::later(page, standing) {
      sections.extend(merged::history(ctx, page, last, shown.base()));
    }
    if standing == Standing::LeftChanged && !page.left_changed.is_empty() {
      sections.push(left::section(ctx, &page.left_changed));
    }
    sections.extend(self.record(ctx, page, shown.base()));
    sections
  }

  /// *Checks* of the last attempt, the passed ones folded, and *Attempts*, each expanding into its own.
  fn record(&self, ctx: &mut Ctx, page: &AttemptPage, base: &str) -> Vec<Element> {
    let branches = Branches {
      base,
      branch: page.branch.as_str(),
    };
    let mut sections = Vec::new();
    if let Some(last) = page.last() {
      let seconds = attempts::to_the_second(&page.attempts);
      let count = checks::heading_count(
        ctx,
        (last.sequence, words::attempt_time(ctx, last.attempt.at(), seconds)),
      );
      sections.push(section(
        "merge-attempt-checks-section",
        section_head(
          "merge-attempt-checks-head",
          &ctx.t("merges.page.checks"),
          (Some(&count), None),
        ),
        checks::checks(
          ctx,
          "merge-attempt-checks",
          (&last.attempt, &branches),
          Folds::Page(&self.shown),
        ),
      ));
    }
    sections.push(section(
      "merge-attempt-attempts-section",
      section_head(
        "merge-attempt-attempts-head",
        &ctx.t("merges.page.attempts"),
        (Some(&page.attempts.len().to_string()), None),
      ),
      attempts::attempts(ctx, (&page.attempts, page.run.number.get()), &branches, &self.expanded),
    ));
    sections
  }
}

/// A section of the body: its heading over its body, 8 apart.
fn section(id: &str, head: Element, body: Element) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(head)
    .child(body)
    .into()
}

/// A section's heading in 13/600, with its count in 12 `text_muted` and, at its end, `end`, a link (`Head`).
fn section_head(id: &str, title: &str, (count, end): (Option<&str>, Option<Element>)) -> Element {
  let count = count.map(|count| {
    Text::new(count)
      .id(format!("{id}-count"))
      .variant(ExtraTypography::Property.role())
      .color(PaletteColor::TextMuted)
      .into()
  });
  section_head_with(id, title, (count, end))
}

/// [`section_head`] with a count that is any element: a bone while it is read.
fn section_head_with(id: &str, title: &str, (count, end): (Option<Element>, Option<Element>)) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(ExtraSpacing::Tight.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(title).variant(ExtraTypography::FormSection.role()).nowrap())
    .with_children(count)
    .child(Row::new().flex(1.0))
    .with_children(end)
    .into()
}

/// The empty state `id` with `glyph`, `title` and `body`: a card 440 wide, centered.
fn empty(id: &str, glyph: Icon, (title, body): (Arc<str>, Arc<str>)) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(DeliverySpacing::MergesEmpty.role()))
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .width(Dimension::Px(EMPTY_WIDTH))
        .child(EmptyState::new(id.to_owned(), glyph, title, body).wide()),
    )
    .into()
}
