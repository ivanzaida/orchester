//! The Review hub (`Review` `QzjsN` in `Hub pages` `ZRBS5`): the Merge queue, the Pull requests and the Merges (09
//! `Os7b3` of `Review · redesign`) as one page. Its head is *Review* (20/600), then the three tabs with their counts,
//! then the open tab's page as it was before the hub. Each tab keeps its own route (`/project/<id>/merge-queue`,
//! `/project/<id>/pull-requests`, `/project/<id>/merges`), so links, history, back and forward and *Jump to* land on
//! it; the sidebar's Review opens the Merge queue. A pull request's own page is not a tab: it keeps its page.
//!
//! The Merge queue counts the runs in the queue; the Pull requests the open ones the stored list holds; the Merges, in
//! `warning`, the runs that need the person there: not merged, and waiting for their review. The counts are read again
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
  tasks::merge_attempts::merges,
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
  Merges,
}

impl ReviewTab {
  const ALL: [Self; 3] = [Self::MergeQueue, Self::PullRequests, Self::Merges];

  /// The tab's name in its element id and the hub's `tab` description.
  const fn name(self) -> &'static str {
    match self {
      Self::MergeQueue => "merge_queue",
      Self::PullRequests => "pull_requests",
      Self::Merges => "merges",
    }
  }

  const fn label_key(self) -> &'static str {
    match self {
      Self::MergeQueue => "shell.screens.mergeQueue",
      Self::PullRequests => "shell.screens.pullRequests",
      Self::Merges => "shell.screens.merges",
    }
  }

  fn route(self, project: ProjectId) -> Route {
    Route::Project(
      project,
      match self {
        Self::MergeQueue => ProjectPage::MergeQueue,
        Self::PullRequests => ProjectPage::PullRequests(PullRequestTab::default()),
        Self::Merges => ProjectPage::Merges,
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
    let queue_reading = feed
      .tasks
      .with(|tasks| matches!(tasks, crate::project::TasksState::Loading));
    let (open, list_reading) = open_pull_requests(ctx, &project);
    let (needing_you, merges_reading) = merges_needing_you(ctx, &project);
    let navigation = Navigation::of(ctx);
    let id = project.open.project.id();
    let tabs: Vec<PageTab> = ReviewTab::ALL
      .into_iter()
      .map(|choice| {
        let (count, reading) = match choice {
          ReviewTab::MergeQueue => (queued, queue_reading),
          ReviewTab::PullRequests => (open, list_reading),
          ReviewTab::Merges => (needing_you, merges_reading),
        };
        let shown = PageTab::to(
          format!("review-hub-tab-{}", choice.name()),
          ctx.t(choice.label_key()),
          choice == tab,
          navigation.to(choice.route(id)),
        );
        match count {
          Some(count) if count > 0 => shown.count(count, choice == ReviewTab::Merges),
          None if reading => shown.count_reading(),
          _ => shown,
        }
      })
      .collect();
    page_tabs(ctx, "review-hub-tabs", tabs)
  }
}

/// The open pull requests the stored list holds, once it was synced, and whether it is read for the first time.
fn open_pull_requests(ctx: &mut Ctx, project: &ShownProject) -> (Option<usize>, bool) {
  let feed = &project.feed;
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
  let reading = listed.is_pending() && listed.data.is_none();
  let open = listed.data.and_then(|list| match list {
    PullRequestList::Linked(linked) if linked.synced_at.is_some() => Some(linked.count(PullRequestTab::Open)),
    _ => None,
  });
  (open, reading)
}

/// The runs that need the person in Merges ([`merges::needing_you`]), and whether they are counted for the first time,
/// the project's tasks still read too.
fn merges_needing_you(ctx: &mut Ctx, project: &ShownProject) -> (Option<usize>, bool) {
  let feed = &project.feed;
  let tasks = feed.tasks.with(|tasks| tasks.ready().cloned());
  let tasks_reading = feed
    .tasks
    .with(|tasks| matches!(tasks, crate::project::TasksState::Loading));
  let deps = (project.open.repo_root.clone(), tasks, feed.saves.get());
  let hold = crate::services::Services::of(ctx).hold;
  let counted = ctx
    .future(deps, move |(root, tasks, _)| {
      let hold = hold.clone();
      async move {
        hold.released().await;
        let Some(tasks) = tasks else {
          return Ok(None);
        };
        background::run(move || merges::needing_you(&tasks, &root))
          .await?
          .map(Some)
          .map_err(|error| {
            tracing::error!(%error, "cannot count the project's runs that need the person in Merges");
            orchester_workspace::messages::store_error(&error)
          })
      }
    })
    .state()
    .get();
  let reading = tasks_reading || (counted.is_pending() && counted.data.is_none());
  (counted.data.flatten(), reading)
}
