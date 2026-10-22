//! A pull request's page (`Pull request · Files changed · dark` `mVTVX`, its states in `c07NAC`; agent feedback in
//! `Pull request feedback` `seN1p`): its header with the state, review, branches, task and checks and how its latest
//! read went; its tabs, of which Files changed and Agent feedback are built (Agent checks and Overview are planned and
//! locked); on Files changed the toolbar with the revision range and the totals, and the changed files beside the
//! selected file's diff with the task page's file list and diff view, feedback threads drawn into it; on Agent
//! feedback the threads and what they ask of the task's agent ([`feedback`]). What it shows and why is
//! `orchester_workspace::pull_requests::page`'s and `::feedback`'s. The page reads the pull request and its diff from
//! GitHub when it opens and on Refresh; what was read before shows meanwhile.

mod body;
mod feedback;
mod header;
mod toolbar;

use std::{num::NonZeroU64, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureHandle},
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::FeedbackId, revision::Revision};
use orchester_exec::external;
use orchester_workspace::{
  activity::operations,
  background,
  messages::Message,
  pull_requests::{
    detail::ReadDiff,
    feedback::{FeedbackBoard, ThreadState},
    page::{PageDiff, PullRequestPage, PullRequestView, between_in, diff_at_in, load_page_in, refresh_page_in},
  },
  workspace::OpenProject,
};

use self::{
  feedback::{BoardRead, Env, FeedbackState, TabProps, Writes, feedback_tab, read_board},
  toolbar::{Fixed, Instead, InsteadWhy},
};
use super::states;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route},
  screens::task::changes::listed::Listed,
  services::Services,
};

type PageAction = FutureAction<(), PullRequestPage, Message>;
type OpenAction = FutureAction<Arc<str>, (), Message>;

/// Which of the page's tabs the route names, with what it names in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shown {
  /// Files changed, on the file whose diff shows.
  Files(Option<Arc<str>>),
  /// Agent feedback, with the thread open beside its list.
  Feedback(Option<FeedbackId>),
}

pub fn screen(ctx: &mut Ctx, project: &ShownProject, number: NonZeroU64, shown: Shown) -> Element {
  ctx.mount_keyed::<PullRequestScreen>(
    &format!("pull-request-{}-{number}", project.open.project.id()),
    Props {
      project: project.clone(),
      number,
      shown,
    },
  )
}

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
  number: NonZeroU64,
  shown: Shown,
}

impl DevtoolsInspectable for Props {}

struct PullRequestScreen {
  refresh: PageAction,
  open: OpenAction,
  /// What the latest read returned; before it, what is stored shows.
  latest: Signal<Option<PullRequestPage>>,
  /// Why the latest read could not run at all.
  failed: Signal<Option<Message>>,
  /// What Find a file narrows the list to.
  find: Signal<String>,
  feedback: FeedbackState,
  writes: Writes,
}

impl Component for PullRequestScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let Props { project, number, .. } = ctx.props::<Props>().clone();
    let refresh = refresh_action(ctx, &project.open, number.get());
    let feedback = FeedbackState::new(ctx);
    let screen = Self {
      open: open_action(ctx, &project.open, number.get()),
      latest: ctx.signal(None),
      failed: ctx.signal(None),
      find: ctx.signal(String::new()),
      refresh: refresh.clone(),
      writes: Writes::new(ctx, (&project.open, number), &feedback),
      feedback,
    };
    let (latest, failed) = (screen.latest.clone(), screen.failed.clone());
    ctx.watch(&refresh.state(), move |state| {
      if state.is_pending() {
        return;
      }
      if let Some(error) = &state.error {
        failed.set(Some(error.clone()));
      } else if let Some(page) = &state.data {
        failed.set(None);
        latest.set(Some(page.clone()));
      }
    });
    refresh.run(());
    screen
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, number, shown } = ctx.props::<Props>().clone();
    let stored = load(ctx, &project.open, number.get()).state().get();
    let page = self.latest.get().or(stored.data);
    let reading = self.refresh.state().get().is_pending();
    let frame = Column::new()
      .id("screen-pull_requests")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0));
    let view = match page {
      Some(PullRequestPage::Linked(view)) => view,
      Some(PullRequestPage::Unlinked) => {
        let name = project.open.project.name().as_str().to_owned();
        return frame
          .child(header::compact(
            ctx,
            (&project, number.get()),
            reading,
            &tabs(&shown, None),
          ))
          .child(states::unavailable(ctx, project.open.project.id(), &name));
      }
      None => {
        return frame
          .child(header::compact(
            ctx,
            (&project, number.get()),
            true,
            &tabs(&shown, None),
          ))
          .with_children(
            stored
              .error
              .or(self.failed.get())
              .map(|error| body::load_failed(ctx, &error)),
          );
      }
    };
    let (head, board) = board_of(ctx, (&project, number), &view, self.feedback.written.get());
    let actions = self.actions(&view);
    let open_count = board.board.as_ref().map(|board| board.open().count());
    let frame = frame
      .child(header::header(
        ctx,
        (&project, &view),
        &actions,
        reading,
        &tabs(&shown, open_count),
      ))
      .with_children(self.failed.get().map(|error| body::load_failed(ctx, &error)));
    match shown {
      Shown::Feedback(selected) => frame.child(feedback_tab(
        ctx,
        &TabProps {
          project,
          number,
          board: board.board,
          failed: board.failed,
          head,
          selected,
          state: self.feedback.clone(),
          writes: self.writes.clone(),
        },
      )),
      Shown::Files(file) => {
        let files = self.files(ctx, (&project, number), (&view, head.as_ref()), (file, &board));
        frame.child(body::body(ctx, &view, &files))
      }
    }
  }
}

/// The page's tabs as the route names them.
fn tabs(shown: &Shown, feedback_count: Option<usize>) -> header::Tabs {
  header::Tabs {
    feedback_shown: matches!(shown, Shown::Feedback(_)),
    feedback_count,
  }
}

/// The pull request's head as last read, and its feedback read against it and its diff.
fn board_of(
  ctx: &mut Ctx,
  (project, number): (&ShownProject, NonZeroU64),
  view: &PullRequestView,
  written: u64,
) -> (Option<Revision>, BoardRead) {
  let head = view
    .pull_request
    .as_ref()
    .map(|pull_request| pull_request.head_sha().clone());
  let latest_diff = match &view.diff {
    PageDiff::Read(read) => Some(read.diff.clone()),
    _ => None,
  };
  let board = read_board(
    ctx,
    (project, number),
    head.clone().map(|head| (head, latest_diff)),
    written,
  );
  (head, board)
}

impl PullRequestScreen {
  fn actions(&self, view: &PullRequestView) -> header::Actions {
    let reading = self.refresh.state().get().is_pending();
    header::Actions {
      refresh: (!reading).then(|| {
        let refresh = self.refresh.clone();
        Arc::new(move || refresh.run(())) as Arc<dyn Fn() + Send + Sync>
      }),
      open: view.pull_request.as_ref().map(|pull_request| {
        let (open, url) = (self.open.clone(), Arc::<str>::from(pull_request.url()));
        Arc::new(move || open.run(url.clone())) as Arc<dyn Fn() + Send + Sync>
      }),
    }
  }

  /// What Files changed is shown with: the file the route names, the diff instead of the latest when one is asked
  /// for, and the feedback once read.
  fn files(
    &self,
    ctx: &mut Ctx,
    (project, number): (&ShownProject, NonZeroU64),
    (view, head): (&PullRequestView, Option<&Revision>),
    (file, board): (Option<Arc<str>>, &BoardRead),
  ) -> body::Shown {
    let navigation = Navigation::of(ctx);
    let id = project.open.project.id();
    let fixed = board.board.as_deref().map(fixed).unwrap_or_default();
    let instead = instead(ctx, (project, number), (&self.feedback, head), &fixed);
    let env = board.board.clone().map(|board| Env {
      project: project.clone(),
      number,
      head: head.cloned(),
      state: self.feedback.clone(),
      writes: self.writes.clone(),
      board,
      navigation: navigation.clone(),
    });
    let reading = self.refresh.state().get().is_pending();
    body::Shown {
      selected: file,
      select: Arc::new(move |listed: &Listed, at| {
        let page = ProjectPage::PullRequest(number, Some(listed.path.to_string()));
        navigation.open(Route::Project(id, page), at);
      }),
      find: self.find.clone(),
      open: view.pull_request.as_ref().map(|pull_request| {
        let (open, url) = (self.open.clone(), Arc::<str>::from(pull_request.url()));
        Arc::new(move || open.run(url.clone())) as Arc<dyn Fn() + Send + Sync>
      }),
      reading,
      instead,
      fixed,
      feedback: env,
      state: self.feedback.clone(),
    }
  }
}

/// The fixes whose commit the range can show what changed since their request.
fn fixed(board: &FeedbackBoard) -> Vec<Fixed> {
  board
    .threads
    .iter()
    .filter_map(|thread| match &thread.state {
      ThreadState::ReadyToVerify { commit, .. } => Some(Fixed {
        thread: thread.id,
        from: thread.current().revision.clone(),
        to: commit.clone(),
      }),
      _ => None,
    })
    .collect()
}

#[derive(Clone, Debug, PartialEq)]
struct Read(Option<Arc<ReadDiff>>);

impl DevtoolsInspectable for Read {}

/// The diff shown instead of the pull request's latest: what changed since a fix's request when the range asks for
/// it, else an earlier head when one is pinned or a new thread is being written at one; read off the UI thread.
fn instead(
  ctx: &mut Ctx,
  (project, number): (&ShownProject, NonZeroU64),
  (state, head): (&FeedbackState, Option<&Revision>),
  fixed: &[Fixed],
) -> Option<Instead> {
  let since = state
    .since
    .get()
    .and_then(|thread| fixed.iter().find(|fixed| fixed.thread == thread))
    .map(|fixed| (fixed.from.clone(), fixed.to.clone()));
  let earlier = state.shown_head(head).filter(|_| since.is_none());
  let writing = earlier.is_some() && state.pinned.get().is_none();
  let programs = Services::of(ctx).programs;
  let deps = (project.open.repo_root.clone(), number, since.clone(), earlier);
  let load = ctx.future(deps, move |(root, number, since, earlier)| {
    let programs = programs.clone();
    async move {
      background::run(move || match (since, earlier) {
        (Some((from, to)), _) => between_in(&root, &programs, (&from, &to)).map_err(|error| {
          tracing::warn!(%error, "cannot read what a fix changed since its request");
          error.message()
        }),
        (None, Some(at)) => diff_at_in(&root, number.get(), &at).map_err(|error| {
          tracing::warn!(%error, "cannot read the pull request's diff at an earlier head");
          error.message()
        }),
        (None, None) => Ok(None),
      })
      .await?
      .map(Read)
    }
  });
  let read = load.state().get().data.and_then(|Read(read)| read)?;
  let why = if since.is_some() {
    InsteadWhy::Since
  } else {
    InsteadWhy::Earlier { writing }
  };
  Some(Instead { read, why })
}

/// The pull request as stored, read once when the page opens.
fn load(ctx: &mut Ctx, open: &OpenProject, number: u64) -> FutureHandle<PullRequestPage, Message> {
  ctx.future((open.repo_root.clone(), number), |(repo_root, number)| async move {
    background::run(move || load_page_in(&repo_root, number))
      .await?
      .map_err(|error| {
        tracing::error!(%error, number, "cannot read the stored pull request");
        error.message()
      })
  })
}

/// Reads the pull request and its diff from GitHub, listed in the status bar as reading it.
fn refresh_action(ctx: &mut Ctx, open: &OpenProject, number: u64) -> PageAction {
  let Services { activity, programs, .. } = Services::of(ctx);
  let open = open.clone();
  ctx.future_action(move |()| {
    let (activity, programs, open) = (activity.clone(), programs.clone(), open.clone());
    async move {
      let repo_root = open.repo_root.clone();
      let work = async move {
        background::run(move || refresh_page_in(&repo_root, number, &programs))
          .await?
          .map_err(|error| {
            tracing::error!(%error, number, "cannot read the pull request");
            error.message()
          })
      };
      activity
        .track(operations::pull_request_refresh(&open, number), work)
        .await
    }
  })
}

fn open_action(ctx: &mut Ctx, open: &OpenProject, number: u64) -> OpenAction {
  let activity = Services::of(ctx).activity;
  let open = open.clone();
  ctx.future_action(move |url: Arc<str>| {
    let (activity, open) = (activity.clone(), open.clone());
    async move {
      let work = async move {
        background::run(move || external::open_url(&url))
          .await?
          .map_err(|error| {
            tracing::error!(%error, number, "cannot open the pull request in the browser");
            orchester_workspace::pull_requests::open_failed()
          })
      };
      activity.track(operations::pull_request_open(&open, number), work).await
    }
  })
}
