//! The Review hub (`Review` `QzjsN` in `Hub pages` `ZRBS5`): the Merge queue and the Pull requests as one page. Its
//! head is *Review* (20/600), then the two tabs with their counts, then the open tab's page as it was before the hub.
//! Each tab keeps its own route (`/project/<id>/merge-queue`, `/project/<id>/pull-requests`), so links, history,
//! back and forward and *Jump to* land on it; the sidebar's Review opens the Merge queue. A pull request's own page
//! is not a tab: it keeps its page.
//!
//! The Merge queue counts the runs in the queue; the Pull requests the open ones the stored list holds, read again
//! when the project's runs or data change. A count of none is left out; one read for the first time is a bone.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::TypographyStyle,
  },
  components::{Column, Row, Text},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  background,
  pull_requests::list::{PullRequestList, PullRequestTab, load_list_in},
};

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route},
  theme::HubSpacing,
  ui::tabs::{PageTab, page_tabs},
};

/// A tab of the Review hub.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReviewTab {
  MergeQueue,
  PullRequests,
}

impl ReviewTab {
  const ALL: [Self; 2] = [Self::MergeQueue, Self::PullRequests];

  /// The tab's name in its element id and the hub's `tab` description.
  const fn name(self) -> &'static str {
    match self {
      Self::MergeQueue => "merge_queue",
      Self::PullRequests => "pull_requests",
    }
  }

  const fn label_key(self) -> &'static str {
    match self {
      Self::MergeQueue => "shell.screens.mergeQueue",
      Self::PullRequests => "shell.screens.pullRequests",
    }
  }

  fn route(self, project: ProjectId) -> Route {
    Route::Project(
      project,
      match self {
        Self::MergeQueue => ProjectPage::MergeQueue,
        Self::PullRequests => ProjectPage::PullRequests(PullRequestTab::default()),
      },
    )
  }
}

/// The hub on `tab` of `project`, with `body`, the tab's page, under the tabs.
pub(crate) fn page(ctx: &mut Ctx, project: &ShownProject, tab: ReviewTab, body: Element) -> Element {
  let head = Column::new()
    .id("review-hub-head")
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(HubSpacing::Side.role()).top(HubSpacing::HeadTop.role()))
    .spacing(HubSpacing::HeadBottom.role())
    .child(
      Text::new(&ctx.t("shell.screens.review"))
        .id("review-hub-title")
        .variant(TypographyStyle::Heading),
    )
    .child(ctx.mount_keyed::<ReviewTabs>(
      &format!("review-hub-tabs-{}", project.open.project.id()),
      Props {
        project: project.clone(),
        tab,
      },
    ));
  Column::new()
    .id("review-hub")
    .describe("tab", tab.name())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(head)
    .child(
      Row::new()
        .id("review-hub-body")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(body),
    )
    .into()
}

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
  tab: ReviewTab,
}

impl DevtoolsInspectable for Props {}

/// The tabs with their counts.
struct ReviewTabs;

impl Component for ReviewTabs {
  type Props = Props;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, tab } = ctx.props::<Props>().clone();
    let feed = &project.feed;
    let queued = feed.tasks.with(|tasks| {
      tasks
        .ready()
        .map(|tasks| orchester_workspace::merge_queue::in_queue(tasks.merge_queue.iter()))
    });
    let queue_reading = feed.tasks.with(|tasks| matches!(tasks, crate::project::TasksState::Loading));
    let deps = (project.open.repo_root.clone(), feed.revision.get(), feed.states.get());
    let hold = crate::services::Services::of(ctx).hold;
    let listed = ctx
      .future(deps, move |(root, ..)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || load_list_in(&root)).await?.map_err(|error| {
            tracing::error!(%error, "cannot count the project's pull requests");
            error.message()
          })
        }
      })
      .state()
      .get();
    let list_reading = listed.is_pending() && listed.data.is_none();
    let open = listed.data.and_then(|list| match list {
      PullRequestList::Linked(linked) if linked.synced_at.is_some() => Some(linked.count(PullRequestTab::Open)),
      _ => None,
    });
    let navigation = Navigation::of(ctx);
    let id = project.open.project.id();
    let tabs: Vec<PageTab> = ReviewTab::ALL
      .into_iter()
      .map(|choice| {
        let (count, reading) = match choice {
          ReviewTab::MergeQueue => (queued, queue_reading),
          ReviewTab::PullRequests => (open, list_reading),
        };
        let shown = PageTab::to(
          format!("review-hub-tab-{}", choice.name()),
          ctx.t(choice.label_key()),
          choice == tab,
          navigation.to(choice.route(id)),
        );
        match count {
          Some(count) if count > 0 => shown.count(count, false),
          None if reading => shown.count_reading(),
          _ => shown,
        }
      })
      .collect();
    page_tabs(ctx, "review-hub-tabs", tabs)
  }
}
