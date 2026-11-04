//! A run's merge attempts (ORC-363; `Review · redesign` 09b `PVZKL`, 09e `s1jIpI`, 09f `Q6yPN8`, 09g `i2rVv`, 09h
//! `rzo3w`, 09j `GhnHQ`, 09k `X3DJKY`, 09l `Asu5i`), under Review › Merges at `/project/<id>/merges/<KEY>/<run>`: the
//! breadcrumb, the task's title with *Copy details* (or the action that moves the run on), *Open task* and the primary
//! action of how its last attempt left it, and the status line; then *What stopped it* (*What happened* once merged) in
//! the words of the stage that ended it, the conflicting files or the files left changed, the last attempt's *Checks*,
//! and *Attempts*, newest first with Orchester's retries, each expanding into its own checks; at the right, the run's
//! properties and links. The page is read from the run's attempts (`merge_attempts::page`) again whenever the project's
//! tasks or runs change, the merge queue moves, a merge job ends, or a button here changed something.

mod actions;
mod attempts;
mod checks;
mod conflicts;
mod details;
pub(super) mod head;
mod left;
mod properties;
pub(crate) mod read;
pub(crate) mod standing;
mod watch;
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
  tasks::{
    integration_jobs::JobState,
    merge_attempts::{page::AttemptPage, read::Retrying},
  },
};

use self::{
  actions::{Actions, Busy, open_folder},
  attempts::Expanded,
  checks::{Branches, Size},
  conflicts::{AskAction, Conflicts, Resolving, StepChoice},
  read::{Read, read},
  standing::Standing,
};
use super::not_merged::try_again;
use crate::{
  navigation::{Navigation, link::Open},
  project::ShownProject,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  services::Services,
  theme::{AttemptSpacing, AttemptText, DeliverySpacing, ExtraSpacing, ExtraTypography, HubSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    code_box::Copier,
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    state::{EmptyState, load_error, loading},
  },
};

/// How long *Copy details* says it copied before it offers to copy again.
const COPY_FEEDBACK: Duration = Duration::from_secs(2);
/// The branch a line names when the run did not record the one it merges into.
const NO_BRANCH: &str = "—";
/// An empty state's card, as Review › Merges' (`jhPCL`).
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
  step: Signal<StepChoice>,
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
      step: ctx.signal(StepChoice::default()),
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
      .map(|page| (page.run.task, page.run.run));
    let feed = &project.feed;
    let resolving = conflicts::read(
      ctx,
      &project,
      wanted,
      (feed.saves.get(), feed.states.get(), self.again.get()),
    );
    let ask = conflicts::ask_action(ctx, &project);
    let frame = Column::new()
      .id("screen-merge-attempt")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(AttemptSpacing::Top.role())
          .left(HubSpacing::Side.role())
          .right(HubSpacing::Side.role()),
      )
      .spacing(DeliverySpacing::Lanes.role())
      .child(head::crumbs(ctx, &project, (&key.to_string(), number.get())));
    let page = match read {
      Read::Reading => {
        return Element::from(frame.child(loading("merge-attempt-reading", &ctx.t("merges.page.reading"))));
      }
      Read::Failed(reason) => {
        let again = self.again.clone();
        let body = ctx.t_args("merges.page.readFailedBody", [("reason", message_text(ctx, &reason))]);
        return frame
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
        return frame
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
    frame.with_children(self.body(ctx, &shown, standing)).into()
  }
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

  /// The page of the run's task, which *Open task* and *Open KEY* open.
  fn task_page(&self, ctx: &mut Ctx) -> Open {
    Navigation::of(ctx).to(Route::Project(
      self.project.open.project.id(),
      ProjectPage::Task(self.page.run.key.clone(), TaskView::of(TaskTab::Summary)),
    ))
  }
}

impl AttemptScreen {
  /// The header, the status line and the columns, as `standing` tells them; with no attempt yet, the header and an
  /// empty state.
  fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>, standing: Option<Standing>) -> Vec<Element> {
    let page = shown.page;
    let actions = Actions::new(
      (&shown.project.open.repo_root, Services::of(ctx).programs, page.run.run),
      ctx.t("task.listSeparator"),
      (self.failed.clone(), self.busy.clone(), self.again.clone()),
    );
    let task = shown.task_page(ctx);
    let open_task = ActionButton::new(
      "merge-attempt-open-task",
      ButtonKind::Secondary,
      ctx.t("merges.page.openTask"),
    )
    .icon(Icon::ArrowUpRight)
    .opens(task.clone())
    .into();
    let Some(standing) = standing else {
      let body = ctx.t_args(
        "merges.page.noAttemptsBody",
        [("run", page.run.number.get().to_string())],
      );
      return vec![
        head::title(&page.run.title, vec![open_task]),
        empty(
          "merge-attempt-none",
          Icon::GitMerge,
          (ctx.t("merges.page.noAttempts"), body),
        ),
      ];
    };
    let mut buttons = vec![self.ghost(ctx, shown, standing, &actions), open_task];
    buttons.extend(self.primary(ctx, shown, standing, &actions));
    let main = self.main(ctx, shown, standing);
    let side = properties::properties(ctx, page, &shown.project.open.repo_root, task);
    let merging = shown.project.feed.job_state.get().run(page.run.run).merge == JobState::Running;
    let watch = merging.then(|| watch::watch(ctx, page.run.run, &self.again));
    vec![
      head::title(&page.run.title, buttons),
      head::status(ctx, page, (standing, shown.base()), watch),
      Row::new()
        .id("merge-attempt-columns")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .padding(Padding::new().top(ExtraSpacing::Loose.role()))
        .spacing(SpacingSize::Section)
        .child(main)
        .child(side)
        .into(),
    ]
  }

  /// The ghost action before *Open task*: *Stop retrying* while Orchester waits to try again, *Open folder* while files
  /// are left changed in the checkout, else *Copy details*.
  fn ghost(&self, ctx: &mut Ctx, shown: &Shown<'_>, standing: Standing, actions: &Actions) -> Element {
    let ghost = |id: &str, key: &str, glyph: Icon, ctx: &Ctx| {
      ActionButton::new(id.to_owned(), ButtonKind::Ghost, ctx.t(key)).icon(glyph)
    };
    let files_left = !shown.page.left_changed.is_empty();
    if !details::offered(standing, files_left) {
      if let Standing::Retrying(_) = standing {
        let actions = actions.clone();
        return ghost(
          "merge-attempt-stop-retrying",
          "merges.page.stopRetrying",
          Icon::CircleStop,
          ctx,
        )
        .on_click(move || actions.stop_retrying())
        .into();
      }
      let folder = shown.project.open.repo_root.clone();
      return ghost(
        "merge-attempt-open-folder",
        "merges.page.openFolder",
        Icon::FolderOpen,
        ctx,
      )
      .on_click(move || open_folder("open-checkout", &folder))
      .into();
    }
    let (glyph, key) = match self.copied.get() {
      Copied::Not => (Icon::Copy, "merges.page.copyDetails"),
      Copied::Done => (Icon::Check, "merges.page.copied"),
      Copied::Failed => (Icon::CircleAlert, "merges.page.copyFailed"),
    };
    let text = details::details(ctx, shown.page, &standing.status(ctx, shown.base()));
    let (copier, copied, reset) = (Copier::of(ctx), self.copied.clone(), self.reset.clone());
    ghost("merge-attempt-copy-details", key, glyph, ctx)
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

  /// The action that moves the run on, as its last attempt left it; none once merged, or while the merge queue tries
  /// again by itself; the merge queue, where the run merges, while it waits for approval there.
  fn primary(&self, ctx: &mut Ctx, shown: &Shown<'_>, standing: Standing, actions: &Actions) -> Option<Element> {
    let button = |id: &str, key: &str, glyph: Icon, ctx: &Ctx| {
      ActionButton::new(id.to_owned(), ButtonKind::Primary, ctx.t(key)).icon(glyph)
    };
    let busy = self.busy.get();
    let pressed = |button: ActionButton, mine: Busy, press: Box<dyn Fn() + Send + Sync>| -> Element {
      match busy {
        Busy::Idle => button.on_click(press).into(),
        current if current == mine => button.loading().into(),
        _ => button.disabled().into(),
      }
    };
    let actions = actions.clone();
    Some(match standing {
      Standing::Merged | Standing::Retrying(Retrying { next: None }) => return None,
      Standing::Retrying(_) => button("merge-attempt-try-now", "merges.action.tryNow", Icon::RotateCcw, ctx)
        .on_click(move || actions.try_now())
        .into(),
      Standing::AwaitingApproval => {
        let queue = Route::Project(shown.project.open.project.id(), ProjectPage::MergeQueue);
        let open = Navigation::of(ctx).to(queue);
        button("merge-attempt-open-queue", "merges.page.openQueue", Icon::ArrowUpRight, ctx)
          .opens(open)
          .into()
      }
      Standing::Conflicts => conflicts::editor_button(ctx, shown.project, shown.page.run.run, shown.resolving),
      Standing::NotRecorded => pressed(
        button(
          "merge-attempt-record-merge",
          "merges.action.recordMerge",
          Icon::Database,
          ctx,
        ),
        Busy::Recording,
        Box::new(move || actions.record_merge()),
      ),
      Standing::LeftChanged if !shown.page.left_changed.is_empty() => pressed(
        button("merge-attempt-put-back", "merges.action.putFilesBack", Icon::Undo2, ctx),
        Busy::PuttingBack,
        Box::new(move || actions.put_files_back()),
      ),
      _ if shown.page.queue_stopped => pressed(
        button(
          "merge-attempt-try-again",
          "merges.action.tryAgain",
          Icon::RotateCcw,
          ctx,
        ),
        Busy::Requeueing,
        Box::new(move || actions.requeue()),
      ),
      _ => try_again(
        ctx,
        shown.project,
        button(
          "merge-attempt-try-again",
          "merges.action.tryAgain",
          Icon::RotateCcw,
          ctx,
        ),
        &shown.page.run,
      ),
    })
  }

  /// The main column, scrolling: what stopped the attempt, what it left, its checks and the attempts.
  fn main(&self, ctx: &mut Ctx, shown: &Shown<'_>, standing: Standing) -> Element {
    let page = shown.page;
    let base = shown.base();
    let what = if standing == Standing::Merged {
      "merges.page.whatHappened"
    } else {
      "merges.page.whatStopped"
    };
    let mut sections: Vec<Element> = Vec::new();
    sections.extend(self.failed.get().map(|reason| {
      Notice::new(
        "merge-attempt-not-done",
        NoticeTone::Danger,
        ctx.t_args("merges.notDone", [("reason", message_text(ctx, &reason))]),
      )
      .into()
    }));
    sections.push(section(
      "merge-attempt-what",
      section_head("merge-attempt-what-head", &ctx.t(what), None),
      Text::new(&standing::explanation(ctx, page, standing, base))
        .id("merge-attempt-explanation")
        .variant(AttemptText::Explanation.role())
        .width(Dimension::Pct(100.0))
        .into(),
    ));
    if let (Standing::Conflicts, Some(AttemptOutcome::Conflicts { files })) =
      (standing, page.last().map(|last| last.attempt.outcome()))
    {
      let conflicts = Conflicts {
        project: shown.project,
        run: page.run.run,
        base,
        files,
        resolving: shown.resolving,
        step: &self.step,
        ask: shown.ask,
        person_only: &self.person_only,
      };
      sections.push(conflicts.render(ctx));
    }
    if standing == Standing::LeftChanged && !page.left_changed.is_empty() {
      sections.push(left::section(ctx, &page.left_changed, base));
    }
    sections.extend(self.record(ctx, page, base));
    Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .height(Dimension::Pct(100.0))
      .child(
        ScrollVertical::new(
          Column::new()
            .width(Dimension::Pct(100.0))
            .padding(Padding::new().right(SpacingSize::Sm).bottom(SpacingSize::Xl))
            .spacing(SpacingSize::Section)
            .with_children(sections),
        )
        .with_scroll_state(self.scroll.clone())
        .id("merge-attempt-main")
        .width(Dimension::Pct(100.0))
        .height(Dimension::Pct(100.0)),
      )
      .into()
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
        section_head("merge-attempt-checks-head", &ctx.t("merges.page.checks"), Some(&count)),
        checks::checks(
          ctx,
          "merge-attempt-checks",
          (&last.attempt, &branches),
          (Size::Page, Some(&self.shown)),
        ),
      ));
    }
    sections.push(section(
      "merge-attempt-attempts-section",
      section_head(
        "merge-attempt-attempts-head",
        &ctx.t("merges.page.attempts"),
        Some(&page.attempts.len().to_string()),
      ),
      attempts::attempts(ctx, (&page.attempts, page.run.number.get()), &branches, &self.expanded),
    ));
    sections
  }
}

/// A section of the main column: its heading over its body, 14 apart.
fn section(id: &str, head: Element, body: Element) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(DeliverySpacing::Lanes.role())
    .child(head)
    .child(body)
    .into()
}

/// A section's heading in 13/600, with its count or what it is about in 12 `text_muted` (`JPcpw`).
fn section_head(id: &str, title: &str, count: Option<&str>) -> Element {
  Row::new()
    .id(id.to_owned())
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(title).variant(ExtraTypography::FormSection.role()))
    .with_children(count.map(|count| {
      Text::new(count)
        .id(format!("{id}-count"))
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::TextMuted)
    }))
    .into()
}

/// The empty state `id` with `glyph`, `title` and `body`: a card 440 wide, centered, as Review › Merges' own.
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
