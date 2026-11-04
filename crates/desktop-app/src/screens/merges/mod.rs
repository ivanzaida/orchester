//! Review › Merges (ORC-362; 09 `Os7b3` of `Review · redesign`): every merge into the project's base branches, read
//! from the merge attempts (`orchester_workspace::tasks::merge_attempts::merges`). Under the info line, *Not merged*:
//! the runs whose last attempt did not land, each with the action that moves it on; *Waiting for your review*: the runs
//! that merge only once a person approves; then the merged runs by local day, newest first, older days read as the list
//! scrolls near its end. Sections without rows are left out; with none at all, *No merges yet* (09c `jhPCL`); while
//! read, bones (09d `Zlk49`). A row's chevron opens the run's attempts ([`attempt`], ORC-363); a run waiting for review
//! opens its review page ([`review`], ORC-697) at the same route. The merges are read again whenever the project's
//! tasks or runs change, a merge job ends, or a button here changed something.

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
mod words;

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
  messages::{self, Message},
  tasks::{
    checkout::Checkout,
    merge_attempts::merges::{self, MergeRun, Merges},
  },
};

use self::actions::{Actions, PuttingBack};
use crate::{
  navigation::{Navigation, link::Open},
  project::ShownProject,
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
    Self {
      again: ctx.signal(0),
      pages: ctx.signal(1),
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
      return frame
        .child(states::empty(
          ctx,
          merges.base.as_ref().map(|base| base.branch.as_str()),
        ))
        .into();
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
  /// The sections with rows, and while earlier merged runs are read, a line that says so.
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

fn read(ctx: &mut Ctx, project: &ShownProject, (again, pages): (u64, usize)) -> Read {
  let feed = &project.feed;
  let tasks = feed.tasks.with(|tasks| tasks.ready().cloned());
  let Some(tasks) = tasks else {
    return Read::Reading;
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
  let pending = loaded.is_pending();
  loaded.data.map_or(Read::Reading, |merges| {
    let listed: usize = merges.merged.iter().map(|day| day.runs.len()).sum();
    Read::Ready {
      pending: pending && merges.more && listed < pages * PAGE,
      merges,
    }
  })
}

/// The page while the merges are read ([`states::reading`]), its lead naming the branch the checkout has (09d `Zlk49`
/// draws the lead real): one quick git call, not held with the merges' reads.
fn reading(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let programs = Services::of(ctx).programs;
  let branch = ctx
    .future(project.open.repo_root.clone(), move |root| {
      let programs = programs.clone();
      async move {
        background::run(move || Checkout::branch(&programs, &root).map(|branch| branch.as_str().to_owned())).await
      }
    })
    .state()
    .get()
    .data
    .flatten();
  states::reading(ctx, branch.as_deref())
}

/// `run`'s page: its review while it waits for one (ORC-697, [`review`]), its merge attempts otherwise (ORC-363,
/// [`attempt`]).
fn attempt_page(ctx: &mut Ctx, project: &ShownProject, run: &MergeRun) -> Open {
  Navigation::of(ctx).to(Route::Project(
    project.open.project.id(),
    ProjectPage::MergeAttempt(run.key.clone(), run.number),
  ))
}
