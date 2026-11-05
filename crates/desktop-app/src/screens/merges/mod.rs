//! Review › Merges (ORC-362; 09 `Os7b3` of `Review · redesign`): every merge into the project's base branches, read
//! from the merge attempts (`orchester_workspace::tasks::merge_attempts::merges`). Under the info line, *Not merged*:
//! the runs whose last attempt did not land, each with the action that moves it on; *Waiting for your review*: the runs
//! that merge only once a person approves; then the merged runs by local day, newest first, older days read as the list
//! scrolls near its end. Sections without rows are left out; with none at all, *No merges yet* (09c `jhPCL`); while
//! read the first time in the window, bones (09d `Zlk49`); opened again, the merges last read, kept in the project's
//! feed, while they are read anew (ORC-721). A row's chevron opens the run's attempts ([`attempt`], ORC-363); a run
//! waiting for review opens its review page ([`review`], ORC-697) at the same route. The merges are read again whenever
//! the project's tasks or runs change, a merge job ends, Orchester's wait to try a merge again begins, changes or ends,
//! or a button here changed something.

mod actions;
pub(crate) mod attempt;
mod header;
mod merged;
mod not_merged;
mod retrying;
pub(crate) mod review;
mod row;
mod states;
mod waiting;
pub(crate) mod words;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::ScrollEvent,
    theme::SpacingSize,
  },
  components::{Column, ScrollVertical},
  core::Signal,
  layout::layout_kind::ScrollState,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  background,
  merge_queue::checkout_base,
  messages::{self, Message},
  tasks::{
    integration_jobs::JobState,
    merge_attempts::merges::{self, MergeRun, Merges},
  },
};

use self::actions::{Actions, PuttingBack};
use crate::{
  navigation::{Navigation, link::Open},
  project::{ShownProject, feed::LastMerges},
  routes::{ProjectPage, Route},
  services::Services,
  theme::{DeliverySpacing, HubSpacing},
  ui::{
    message::message_text,
    notice::{Notice, NoticeTone},
    state::{load_error, loading},
  },
};

/// How many merged runs are read at first, and how many more each time the list scrolls near its end.
const PAGE: usize = 30;
/// How near its end the list reads the next merged runs.
const NEAR_END: f32 = 240.0;

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
}

impl DevtoolsInspectable for Props {}

/// Review › Merges of `project`.
pub fn screen(ctx: &mut Ctx, project: &ShownProject) -> Element {
  ctx.mount_keyed::<MergesScreen>(
    &format!("merges-{}", project.open.project.id()),
    Props {
      project: project.clone(),
    },
  )
}

struct MergesScreen {
  /// Counts Try again after a failed read, and the changes this screen's buttons made.
  again: Signal<u64>,
  /// How many pages of merged runs are read.
  pages: Signal<usize>,
  failed: Signal<Option<Message>>,
  putting_back: Signal<PuttingBack>,
  scroll: ScrollState,
}

impl Component for MergesScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    // Opened again, it reads as many pages as it shows from the last read, so the list does not shrink as they land.
    let pages = ctx
      .props::<Props>()
      .project
      .feed
      .merges
      .with_untracked(|last| last.as_ref().map_or(1, |last| last.pages));
    Self {
      again: ctx.signal(0),
      pages: ctx.signal(pages),
      failed: ctx.signal(None),
      putting_back: ctx.signal(PuttingBack::default()),
      scroll: ScrollState::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project } = ctx.props::<Props>().clone();
    let frame = Column::new()
      .id("screen-merges")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(DeliverySpacing::Lanes.role())
          .left(HubSpacing::Side.role())
          .right(HubSpacing::Side.role()),
      )
      .spacing(DeliverySpacing::Lanes.role());
    let pages = self.pages.get();
    let (merges, more_pending) = match read(ctx, &project, (self.again.get(), pages)) {
      Read::Reading => return Element::from(frame.child(reading(ctx, &project))),
      Read::Failed(reason) => {
        let again = self.again.clone();
        let body = ctx.t_args("merges.readFailedBody", [("reason", message_text(ctx, &reason))]);
        return frame
          .child(load_error(
            ctx,
            "merges-load-error",
            (&ctx.t("merges.readFailed"), &body),
            move || again.update(|count| *count += 1),
          ))
          .into();
      }
      Read::Ready { merges, pending } => (merges, pending),
    };
    let frame = frame
      .child(header::info(ctx, merges.base.as_ref()))
      .with_children(self.failed.get().map(|reason| {
        Notice::new(
          "merges-not-done",
          NoticeTone::Danger,
          ctx.t_args("merges.notDone", [("reason", message_text(ctx, &reason))]),
        )
      }));
    if merges.is_empty() {
      let empty = states::empty(ctx, merges.base.as_ref().map(|base| base.branch.as_str()));
      return frame.child(empty).with_children(self.watches(ctx, &project)).into();
    }
    let body = self.body(ctx, &project, &merges, more_pending);
    let more = (merges.more && !more_pending).then(|| self.read_more(pages));
    let scroll = self.scroll.clone();
    frame
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .child(
            ScrollVertical::new(
              Column::new()
                .width(Dimension::Pct(100.0))
                .padding(Padding::new().right(SpacingSize::Sm).bottom(SpacingSize::Xl))
                .spacing(DeliverySpacing::Lanes.role())
                .with_children(body),
            )
            .with_scroll_state(self.scroll.clone())
            .id("merges-main")
            .width(Dimension::Pct(100.0))
            .height(Dimension::Pct(100.0))
            .on_scroll(move |event: ScrollEvent| {
              // A wheel's handlers run before its delta moves the list: the offset it leads to is less the delta.
              let end = (scroll.content_height() - scroll.viewport_height()).max(0.0);
              let at = (scroll.scroll_y() - event.delta_y).clamp(0.0, end);
              if let Some(more) = &more
                && end - at < NEAR_END
              {
                more();
              }
            }),
          ),
      )
      .into()
  }
}

impl MergesScreen {
  /// Orchester's waits between its tries change no job state: each run whose merge job works is watched, so its row
  /// turns to *Retrying by itself* and its attempt stays current as they begin, change and end. They render nothing.
  fn watches(&self, ctx: &mut Ctx, project: &ShownProject) -> Vec<Element> {
    let jobs = project.feed.job_state.get();
    jobs
      .runs()
      .filter(|(_, state)| state.merge == JobState::Running)
      .map(|(run, _)| attempt::watch::watch(ctx, run, &self.again))
      .collect()
  }

  /// The sections with rows, while earlier merged runs are read a line that says so, and the [`Self::watches`].
  fn body(&self, ctx: &mut Ctx, project: &ShownProject, merges: &Merges, more_pending: bool) -> Vec<Element> {
    let separator = ctx.t("task.listSeparator");
    let services = Services::of(ctx);
    let actions = Actions::new(
      (&project.open.repo_root, services.programs),
      separator,
      (self.failed.clone(), self.putting_back.clone(), self.again.clone()),
    );
    let mut body: Vec<Element> = Vec::new();
    body.extend(not_merged::section(ctx, project, &actions, &merges.not_merged));
    body.extend(waiting::section(ctx, project, &merges.waiting));
    body.extend(merged::days(ctx, project, &merges.merged));
    if more_pending {
      body.push(loading("merges-reading-more", &ctx.t("merges.readingMore")).into());
    }
    body.extend(self.watches(ctx, project));
    body
  }

  /// Reads one more page of merged runs than the `pages` read, once.
  fn read_more(&self, pages: usize) -> impl Fn() + Send + Sync + 'static {
    let shown = self.pages.clone();
    move || {
      if shown.get_untracked() == pages {
        shown.set(pages + 1);
      }
    }
  }
}

/// What the screen shows of the merges.
#[derive(Clone, Debug, PartialEq)]
enum Read {
  Reading,
  /// Read, and whether earlier merged runs, a page more than these hold, are being read.
  Ready {
    merges: Merges,
    pending: bool,
  },
  Failed(Message),
}

/// The merges as last read: on a fresh mount, while the first read of this mount is under way, the ones the project's
/// feed kept from the screen's last read in this window, read with as many `pages`; each read that lands is kept there.
fn read(ctx: &mut Ctx, project: &ShownProject, (again, pages): (u64, usize)) -> Read {
  let feed = &project.feed;
  let kept = || {
    feed
      .merges
      .get_untracked()
      .filter(|last| last.pages == pages)
      .map(|last| last.merges)
  };
  let tasks = feed.tasks.with(|tasks| tasks.ready().cloned());
  let Some(tasks) = tasks else {
    return kept().map_or(Read::Reading, |merges| ready(merges, false, pages));
  };
  let root = project.open.repo_root.clone();
  let queue = crate::project::merge_queue::changes(ctx, &root);
  let changes = (queue, feed.saves.get(), feed.states.get(), feed.job_state.get(), again);
  let services = Services::of(ctx);
  let (programs, hold) = (services.programs, services.hold);
  let loaded = ctx
    .future((root, tasks, pages, changes), move |(root, tasks, pages, _)| {
      let (programs, hold) = (programs.clone(), hold.clone());
      async move {
        hold.released().await;
        background::run(move || merges::merges(&tasks, (&root, &programs), pages * PAGE))
          .await?
          .map_err(|error| messages::store_error(&error))
      }
    })
    .state()
    .get();
  if let Some(error) = loaded.error.clone().filter(|_| loaded.is_rejected()) {
    return Read::Failed(error);
  }
  if let Some(merges) = loaded.fulfilled_data()
    && feed.merges.with_untracked(|last| {
      last
        .as_ref()
        .is_none_or(|last| last.pages != pages || last.merges != *merges)
    })
  {
    feed.merges.set(Some(LastMerges {
      merges: merges.clone(),
      pages,
    }));
  }
  let pending = loaded.is_pending();
  loaded
    .data
    .or_else(kept)
    .map_or(Read::Reading, |merges| ready(merges, pending, pages))
}

/// `merges` as read with `pages`, `pending` while a read is under way.
fn ready(merges: Merges, pending: bool, pages: usize) -> Read {
  let listed: usize = merges.merged.iter().map(|day| day.runs.len()).sum();
  Read::Ready {
    pending: pending && merges.more && listed < pages * PAGE,
    merges,
  }
}

/// The page while the merges are read ([`states::reading`]), its info line drawn from how git tells the checkout
/// stands (09d `Zlk49` draws the lead real): the same call the merges' read makes for it, quick and not held with the
/// merges' reads, so the line does not move when they land. `None` until git answered.
fn reading(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let programs = Services::of(ctx).programs;
  let told = ctx
    .future(project.open.repo_root.clone(), move |root| {
      let programs = programs.clone();
      async move { background::run(move || checkout_base(&root, &programs)).await }
    })
    .state()
    .get();
  // A git that could not be asked tells no branch, as the merges' read then does.
  let base = (!told.is_pending()).then(|| told.data.flatten());
  states::reading(ctx, base.as_ref().map(Option::as_ref))
}

/// `run`'s page: its review while it waits for one (ORC-697, [`review`]), its merge attempts otherwise (ORC-363,
/// [`attempt`]).
fn attempt_page(ctx: &mut Ctx, project: &ShownProject, run: &MergeRun) -> Open {
  Navigation::of(ctx).to(Route::Project(
    project.open.project.id(),
    ProjectPage::MergeAttempt(run.key.clone(), run.number),
  ))
}
