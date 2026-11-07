//! A task's review (ORC-795, ORC-697; `Review - v2` 02 `gSMiI`, the thread and tray states of `Review · redesign`
//! `RBR9O`), at `/project/<id>/review/<KEY>`: the task's latest run that waits for review, was sent back from it
//! (Implement works on comments sent from here) or was approved, or, when it has none, that there is nothing to review
//! for it. An approved run (ORC-800, 07 `eidig`) is read-only ([`approved`]): merged or merging, with its diff and
//! threads kept, and no **+**, comment box, tray, *Approve and merge*, *Viewed* mark or thread action. The
//! breadcrumb, the title with *Open task* and *Approve and merge*, the status line; at the left the run's files with
//! their *Viewed* marks; beside them the run's real diff against its base with the threads under their lines
//! ([`diff`], [`thread`]) and the comment box ([`composer`]); under it the send tray while comments wait ([`tray`]),
//! and the agent bar while Implement works on comments sent to it ([`agent`], `Review - v2` 06 `gwE2q`).
//! The page is read again whenever the project's tasks or runs change state or a change of it landed, and the review
//! alone on every change of the project's runs, so an answer streams in. Esc stops an answer under way. Where the
//! branch moved after the head reviewed, a strip under the status line switches the diff between all changes and those
//! since the review ([`since`]).

mod actions;
mod agent;
mod approved;
mod composer;
mod conflict;
mod crumbs;
mod diff;
mod files;
mod head;
mod pager;
mod read;
mod reading;
mod since;
mod state;
mod summary;
mod tag;
mod thread;
mod tray;
mod words;

use std::{
  collections::{BTreeSet, HashMap},
  hash::{DefaultHasher, Hash, Hasher},
  sync::Arc,
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::KeyboardEvent,
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  diff::Diff,
  project::TaskKey,
  review::{ThreadKind, ThreadState},
  revision::Revision,
  run::RunNumber,
};
use orchester_workspace::tasks::{
  changes::Checkpointed,
  review_list::Pager,
  review_threads::{page::ReviewPage, read::Review},
};
use parking_lot::Mutex;

use self::{
  actions::{Write, Writes},
  crumbs::crumbs,
  diff::{DiffProps, ReviewDiff},
  read::{Read, read},
  since::Shown,
  state::ReviewState,
};
use super::list::read::last_read;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  theme::{AttemptSpacing, ExtraSpacing, HubSpacing},
  ui::{
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    state::{EmptyState, load_error},
  },
};

/// The review of the task `key` in `project`: its latest run that waits for review or was sent back from it.
pub(crate) fn screen(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey) -> Element {
  ctx.mount_keyed::<TaskScreen>(
    &format!("task-review-{}-{key}", project.open.project.id()),
    Props {
      project: project.clone(),
      key: key.clone(),
    },
  )
}

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
  key: TaskKey,
}

impl DevtoolsInspectable for Props {}

struct TaskScreen {
  /// Counts Try again after a failed read and the changes the page made.
  again: Signal<u64>,
  /// Where the task stood in *Waiting for you* when the page last saw it there: once it left, the page stays on it.
  stood: Mutex<Option<usize>>,
}

impl Component for TaskScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      again: ctx.signal(0),
      stood: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, key } = ctx.props::<Props>().clone();
    let waiting = pager::of(last_read(ctx, &project).as_ref(), &key, &mut self.stood.lock());
    let page = match read(ctx, &project, &key, self.again.get()) {
      Read::Ready(page) => page,
      Read::Reading => return reading::page(ctx, &project, &key, waiting.as_ref()),
      Read::Nothing => {
        let nothing = EmptyState::new(
          "review-no-run",
          Icon::FileDiff,
          ctx.t_args("review_page.noRun", [("key", key.to_string())]),
          ctx.t("review_page.noRunBody"),
        )
        .wide();
        return frame(ctx, &project, (&key, None), waiting.as_ref())
          .child(nothing)
          .into();
      }
      Read::Failed(reason) => {
        let again = self.again.clone();
        let body = ctx.t_args("review_page.readFailedBody", [("reason", message_text(ctx, &reason))]);
        let failed = load_error(
          ctx,
          "review-load-error",
          (&ctx.t("review_page.readFailed"), &body),
          move || again.update(|count| *count += 1),
        );
        return frame(ctx, &project, (&key, None), waiting.as_ref())
          .child(failed)
          .into();
      }
    };
    ctx.mount_keyed::<ReviewScreen>(
      &format!("review-{}", page.run.run),
      BodyProps {
        project,
        page,
        waiting,
        written: self.again.clone(),
      },
    )
  }
}

/// The page's frame with its breadcrumb: 20 over it, 28 at its sides, its parts 12 apart (`W31YR`).
fn frame(ctx: &mut Ctx, project: &ShownProject, run: (&TaskKey, Option<RunNumber>), waiting: Option<&Pager>) -> Column {
  let column = Column::new()
    .id("screen-review")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(AttemptSpacing::Top.role())
        .left(HubSpacing::Side.role())
        .right(HubSpacing::Side.role())
        .bottom(SpacingSize::Lg),
    )
    .spacing(SpacingSize::Md)
    .child(crumbs(ctx, project, run, waiting));
  match waiting {
    Some(waiting) => column.on_key_down(pager::keys(Navigation::of(ctx), project, waiting)),
    None => column,
  }
}

#[derive(Clone)]
struct BodyProps {
  project: ShownProject,
  page: Arc<ReviewPage>,
  waiting: Option<Pager>,
  written: Signal<u64>,
}

impl PartialEq for BodyProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.page == other.page
      && self.waiting == other.waiting
      && self.written.id() == other.written.id()
  }
}

impl DevtoolsInspectable for BodyProps {}

/// A file's open threads by kind, and its outdated ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ThreadCounts {
  pub comments: usize,
  pub questions: usize,
  pub outdated: usize,
}

/// What the page's parts are drawn with.
#[derive(Clone)]
struct Env {
  pub project: ShownProject,
  pub page: Arc<ReviewPage>,
  /// The review as read last, the live read's once there is one.
  pub review: Arc<Review>,
  /// Where the run's branch is: the head shown and reviewed.
  pub head: Revision,
  pub diff: Arc<Diff>,
  pub state: ReviewState,
  pub writes: Writes,
  pub navigation: Navigation,
  /// A person approved the run: nothing on the page changes anything.
  pub read_only: bool,
  /// The files marked *Viewed* at `head`.
  pub viewed: Arc<BTreeSet<String>>,
  pub counts: Arc<HashMap<Arc<str>, ThreadCounts>>,
  /// What the diff column shows: the whole diff, or what changed since the review.
  pub shown: Shown,
  /// Tells what this was made of apart from what it was made of before: the page and the review read.
  pub version: u64,
}

impl Env {
  /// This for the rows of `diff`, the diff shown: they name their files by position in it, and draw again when it is
  /// another one.
  fn showing(&self, diff: &Arc<Diff>) -> Self {
    let mut hasher = DefaultHasher::new();
    (self.version, Arc::as_ptr(diff) as usize).hash(&mut hasher);
    Self {
      diff: diff.clone(),
      version: hasher.finish(),
      ..self.clone()
    }
  }
}

struct ReviewScreen {
  state: ReviewState,
  writes: Writes,
  /// The review the page read last, shared while it stays the one shown.
  read_with_page: Mutex<Option<(Arc<ReviewPage>, Arc<Review>)>>,
}

impl Component for ReviewScreen {
  type Props = BodyProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<BodyProps>().clone();
    let state = ReviewState::new(ctx, props.written.clone());
    let writes = Writes::new(ctx, &props.project, props.page.run.run, &state);
    Self {
      state,
      writes,
      read_with_page: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let BodyProps {
      project, page, waiting, ..
    } = ctx.props::<BodyProps>().clone();
    let key = (page.run.key.clone(), page.run.number);
    let live = read::live(ctx, &project, page.run.run, self.state.written.get());
    let wanted = since::moved(&page).is_some() && since::wanted(&project, &key.0);
    let shown = Shown::of(read::since(
      ctx,
      &project,
      (page.reviewed.clone(), page.review.head.clone()),
      wanted,
    ));
    let frame = frame(ctx, &project, (&key.0, Some(key.1)), waiting.as_ref());
    let (Some(head), Checkpointed::Read { changes, .. }) = (page.review.head.clone(), &page.changes) else {
      return Element::from(frame.child(self.nothing(ctx, &page)));
    };
    let review = live
      .filter(|review| review.head == page.review.head)
      .unwrap_or_else(|| self.page_review(&page));
    let env = Env {
      version: version(&page, &review),
      counts: Arc::new(counts(&review)),
      shown,
      viewed: Arc::new(review.viewed.iter().map(|viewed| viewed.path.clone()).collect()),
      diff: changes.diff.clone(),
      navigation: Navigation::of(ctx),
      read_only: page.approved.is_some(),
      writes: self.writes.clone(),
      state: self.state.clone(),
      project,
      page: page.clone(),
      review,
      head,
    };
    let body = self.body(ctx, &env, changes.key.clone());
    let frame = if env.read_only {
      frame
    } else {
      frame.on_key_down(stop_on_escape(&env))
    };
    frame
      .child(head::header(ctx, &env))
      .child(head::status(ctx, &env))
      .with_children(
        since::strip(ctx, &env)
          .or_else(|| env.page.conflicts.is_none().then(|| summary::summary(ctx, &env)).flatten()),
      )
      .with_children(self.state.failure.get().map(|reason| -> Element {
        Notice::new("review-not-done", NoticeTone::Danger, message_text(ctx, &reason)).into()
      }))
      .child(body)
      .into()
  }
}

impl ReviewScreen {
  /// The files beside the diff, the tray under it.
  fn body(&self, ctx: &mut Ctx, env: &Env, key: Arc<str>) -> Element {
    let (diff, key) = match &env.shown {
      Shown::Since(changes) => (changes.diff.clone(), changes.key.clone()),
      _ => (env.diff.clone(), key),
    };
    let diff = since::instead(ctx, &env.shown).unwrap_or_else(|| {
      let shown = env.showing(&diff);
      ctx.mount_keyed::<ReviewDiff>(
        "review-diff-view",
        DiffProps {
          env: Arc::new(shown),
          diff,
          key,
        },
      )
    });
    Row::new()
      .id("review-columns")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Tabs.role())
      .child(files::files(ctx, env, &env.diff))
      .child(
        Column::new()
          .id("review-main")
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .height(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Loose.role())
          .child(diff)
          .with_children(tray::tray(ctx, env))
          .with_children(agent::bar(ctx, env))
          .with_children(conflict::bar(ctx, env)),
      )
      .into()
  }

  /// The run has nothing to review here: no checkpoint yet, or a diff larger than Orchester reads.
  fn nothing(&self, ctx: &Ctx, page: &ReviewPage) -> Element {
    let (title, body) = match &page.changes {
      Checkpointed::TooLarge { size, .. } => (
        ctx.t("review_page.tooLarge"),
        ctx.t_args(
          "review_page.tooLargeBody",
          [
            ("files", size.files.to_string()),
            ("branch", page.branch.as_str().to_owned()),
          ],
        ),
      ),
      _ => (ctx.t("review_page.nothing"), ctx.t("review_page.nothingBody")),
    };
    EmptyState::new("review-nothing", Icon::FileDiff, title, body)
      .wide()
      .into()
  }

  /// The review `page` was read with, the same `Arc` while the page stays.
  fn page_review(&self, page: &Arc<ReviewPage>) -> Arc<Review> {
    let mut kept = self.read_with_page.lock();
    if let Some((read, review)) = kept.as_ref()
      && Arc::ptr_eq(read, page)
    {
      return review.clone();
    }
    let review = Arc::new(page.review.clone());
    *kept = Some((page.clone(), review.clone()));
    review
  }
}

/// Esc stops the answer under way, unless a text field has the key.
fn stop_on_escape(env: &Env) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  let writes = env.writes.clone();
  let answering = env
    .review
    .threads
    .iter()
    .rev()
    .find(|view| thread::answering(view))
    .map(|view| view.thread.id());
  move |event: KeyboardEvent| {
    if event.key != "Escape" || event.default_prevented() || event.text_input_focused {
      return;
    }
    if let Some(thread) = answering {
      event.prevent_default();
      writes.run(Write::Stop(thread));
    }
  }
}

/// Each file's open comments and questions and its outdated threads, by path.
fn counts(review: &Review) -> HashMap<Arc<str>, ThreadCounts> {
  let mut counts: HashMap<Arc<str>, ThreadCounts> = HashMap::new();
  for view in &review.threads {
    let count = counts.entry(Arc::from(view.thread.anchor().path())).or_default();
    match (&view.state, view.thread.kind()) {
      (ThreadState::Outdated { .. }, _) => count.outdated += 1,
      (ThreadState::Resolved, _) => {}
      (_, ThreadKind::Comment) => count.comments += 1,
      (_, ThreadKind::Question) => count.questions += 1,
    }
  }
  counts
}

/// What tells the page and review read apart from those read before.
fn version(page: &Arc<ReviewPage>, review: &Arc<Review>) -> u64 {
  let mut hasher = DefaultHasher::new();
  (Arc::as_ptr(page) as usize).hash(&mut hasher);
  (Arc::as_ptr(review) as usize).hash(&mut hasher);
  hasher.finish()
}
