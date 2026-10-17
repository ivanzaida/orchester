//! A pull request's page (`Pull request · Files changed · dark` `mVTVX`, its states in `c07NAC`): its header
//! with the state, review, branches, task and checks and how its latest read went; its tabs, of which only
//! Files changed is built (Agent feedback, Agent checks and Overview, `whwRb`, are planned and locked); the
//! toolbar with the revision range (choosing another is planned) and the totals; and the changed files beside
//! the selected file's diff, with the task page's file list and diff view. What it shows and why is
//! `orchester_workspace::pull_requests::page`'s. The page reads the pull request and its diff from GitHub when
//! it opens and on Refresh; what was read before shows meanwhile.

mod body;
mod header;

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
use orchester_exec::external;
use orchester_workspace::{
  activity::operations,
  background,
  messages::Message,
  pull_requests::page::{PullRequestPage, load_page_in, refresh_page_in},
  workspace::OpenProject,
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

pub fn screen(ctx: &mut Ctx, project: &ShownProject, number: NonZeroU64, file: Option<&str>) -> Element {
  ctx.mount_keyed::<PullRequestScreen>(
    &format!("pull-request-{}-{number}", project.open.project.id()),
    Props {
      project: project.clone(),
      number,
      file: file.map(Arc::from),
    },
  )
}

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
  number: NonZeroU64,
  /// The file whose diff shows, as the route names it.
  file: Option<Arc<str>>,
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
}

impl Component for PullRequestScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let Props { project, number, .. } = ctx.props::<Props>().clone();
    let refresh = refresh_action(ctx, &project.open, number.get());
    let screen = Self {
      open: open_action(ctx, &project.open, number.get()),
      latest: ctx.signal(None),
      failed: ctx.signal(None),
      find: ctx.signal(String::new()),
      refresh: refresh.clone(),
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
    let Props {
      project,
      number: page_number,
      file,
    } = ctx.props::<Props>().clone();
    let number = page_number.get();
    let stored = load(ctx, &project.open, number).state().get();
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
          .child(header::compact(ctx, &project, number, reading))
          .child(states::unavailable(ctx, project.open.project.id(), &name));
      }
      None => {
        return frame.child(header::compact(ctx, &project, number, true)).with_children(
          stored
            .error
            .or(self.failed.get())
            .map(|error| body::load_failed(ctx, &error)),
        );
      }
    };
    let actions = header::Actions {
      refresh: (!reading).then(|| {
        let refresh = self.refresh.clone();
        Arc::new(move || refresh.run(())) as Arc<dyn Fn() + Send + Sync>
      }),
      open: view.pull_request.as_ref().map(|pull_request| {
        let (open, url) = (self.open.clone(), Arc::<str>::from(pull_request.url()));
        Arc::new(move || open.run(url.clone())) as Arc<dyn Fn() + Send + Sync>
      }),
    };
    let navigation = Navigation::of(ctx);
    let id = project.open.project.id();
    let shown = body::Shown {
      selected: file,
      select: Arc::new(move |listed: &Listed, at| {
        let page = ProjectPage::PullRequest(page_number, Some(listed.path.to_string()));
        navigation.open(Route::Project(id, page), at);
      }),
      find: self.find.clone(),
      open: actions.open.clone(),
      reading,
    };
    frame
      .child(header::header(ctx, &project, &view, &actions, reading))
      .with_children(self.failed.get().map(|error| body::load_failed(ctx, &error)))
      .child(body::body(ctx, &view, &shown))
  }
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
