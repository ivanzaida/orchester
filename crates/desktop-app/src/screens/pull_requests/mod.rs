//! The project's Pull requests page (`Pull requests` `l0YNS`: `p9R7P` inbox, `RBE9F` a tab's part, `YtGfF` an
//! empty tab, `y7Nwqt` Merged, `nG8p5` unavailable, `uuaob` sync states, `WUOYP` account states): the pull
//! requests of the project's linked GitHub repository by tab, each with its state and review, its checks and
//! the task whose run made its branch. What they are, which tab holds which and what a sync came to are
//! `orchester_workspace::pull_requests`'s; the page shows them.
//!
//! Sync follows `docs/design/orchester-workspace.md`, "GitHub", rule 9: when the page opens, on Refresh, and
//! every minute while it is shown; the timer lives with the page, so nothing polls once another screen
//! replaces it. A failed sync keeps the last list under a notice; a rate limit holds syncs back until it
//! resets. A pull request's own page is `detail`.

mod detail;
mod header;
mod labels;
mod notice;
mod query;
mod row;
mod states;
mod status;
mod table;
mod work;

use std::{num::NonZeroU64, sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::time::Timestamp;
use orchester_workspace::{
  messages::Message,
  pull_requests::list::{LinkedList, PullRequestList, PullRequestSort, PullRequestTab},
};

use self::{
  header::{Linked, Refresh},
  notice::NoticeActions,
  query::Choices,
  status::Shown,
  table::Rows,
  work::Actions,
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsView, ProjectPage, Route},
  theme::{ExtraSpacing, PullRequestsSpacing},
  ui::{
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// Automatic sync while the page is shown (design rule 9).
const SYNC_INTERVAL: Duration = Duration::from_secs(60);

pub fn screen(ctx: &mut Ctx, project: &ShownProject, tab: PullRequestTab) -> Element {
  ctx.mount_keyed::<PullRequestsScreen>(
    &format!("pull-requests-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      tab,
    },
  )
}

/// Pull request `number`'s own page, showing the diff of `file` (the first when `None`).
pub fn pull_request(ctx: &mut Ctx, project: &ShownProject, number: NonZeroU64, file: Option<&str>) -> Element {
  detail::screen(ctx, project, number, file)
}

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
  tab: PullRequestTab,
}

impl DevtoolsInspectable for Props {}

struct PullRequestsScreen {
  /// What the latest sync or *Use this account* returned; before it, the stored list is shown.
  latest: Signal<Option<PullRequestList>>,
  /// Why the latest sync could not run at all (storage, a stopped worker).
  failed: Signal<Option<Message>>,
  /// Asks for the stored list again after reading it failed.
  reread: Signal<u64>,
  choices: Choices,
  /// Made once, in `create`: the first sync starts there, and a render's own futures are its stored list's.
  actions: Actions,
}

impl Component for PullRequestsScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let open = ctx.props::<Props>().project.open.clone();
    let actions = Actions::of(ctx, &open);
    let screen = Self {
      latest: ctx.signal(None),
      failed: ctx.signal(None),
      reread: ctx.signal(0),
      choices: Choices {
        search: ctx.signal(String::new()),
        sort: ctx.signal(PullRequestSort::default()),
      },
      actions: actions.clone(),
    };
    actions.watch(ctx, &screen.latest, &screen.failed);
    actions.sync.run(open.repo_root.clone());
    let (latest, repo_root) = (screen.latest.clone(), open.repo_root);
    ctx
      .create_interval(SYNC_INTERVAL, move || {
        let paused = matches!(
          latest.get_untracked(),
          Some(PullRequestList::Linked(linked)) if linked.is_paused(Timestamp::now())
        );
        if !paused && !actions.running_now() {
          actions.sync.run(repo_root.clone());
        }
      })
      .start();
    screen
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, tab } = ctx.props::<Props>().clone();
    let actions = self.actions.clone();
    let stored = work::load(ctx, &project.open, self.reread.get());
    let loaded = stored.state().get();
    let pending = loaded.is_pending();
    let list = self.latest.get().or(loaded.data);
    let syncing = actions.running();
    let now = Timestamp::now();
    let page = frame();
    let Some(list) = list else {
      // Until the stored list is known only the heading shows, so nothing appears that moves once it is.
      let page = page.child(header::header(ctx, None, &Shown::Syncing, now));
      return match loaded.error {
        Some(error) if !pending => {
          let reread = self.reread.clone();
          page.child(states::failed(ctx, &error, move || reread.update(|count| *count += 1)))
        }
        _ => page,
      };
    };
    let PullRequestList::Linked(linked) = list else {
      let name = project.open.project.name().as_str().to_owned();
      return page
        .child(header::header(ctx, None, &Shown::NotConnected, now))
        .child(states::unavailable(ctx, project.open.project.id(), &name));
    };
    let shown = if syncing {
      Shown::Syncing
    } else {
      Shown::After(linked.status())
    };
    let refresh = self.refresh(&actions, &project, &linked, syncing);
    let page = page.child(header::header(
      ctx,
      Some(Linked {
        repo: &linked.repo,
        refresh,
      }),
      &shown,
      now,
    ));
    self.body(ctx, page, (&project, tab), (&linked, &actions), now)
  }
}

impl PullRequestsScreen {
  /// What Refresh does: syncs, unless a sync runs or a rate limit holds syncs back.
  fn refresh(&self, actions: &Actions, project: &ShownProject, linked: &LinkedList, syncing: bool) -> Refresh {
    if syncing {
      return Refresh::Syncing;
    }
    if linked.is_paused(Timestamp::now()) {
      return Refresh::RateLimited;
    }
    let (sync, root) = (actions.sync.clone(), project.open.repo_root.clone());
    Refresh::Ready(Arc::new(move || sync.run(root.clone())))
  }

  /// The tabs, the failed sync's notice, the query and the tab's rows with their count, or why there is
  /// none.
  fn body(
    &self,
    ctx: &mut Ctx,
    page: Column,
    (project, tab): (&ShownProject, PullRequestTab),
    (linked, actions): (&LinkedList, &Actions),
    now: Timestamp,
  ) -> Column {
    let id = project.open.project.id();
    let known = linked.synced_at.is_some();
    let page = page.child(query::tabs(ctx, id, tab, known.then_some(linked)));
    let notice_actions = self.notice_actions(ctx, project, actions);
    let notice = notice::failure_notice(ctx, linked, project.open.project.name().as_str(), now, &notice_actions);
    let failed = self.failed.get().map(|reason| {
      Element::from(
        Notice::new(
          "pull-requests-sync-failed",
          NoticeTone::Danger,
          ctx.t("pull_requests.syncFailed"),
        )
        .body(message_text(ctx, &reason)),
      )
    });
    // A notice goes under the search, over the rows it is about, as on the Tasks page.
    let query = query::query(ctx, &self.choices);
    let page = page.child(query).with_children(failed).with_children(notice);
    if !known {
      // Nothing was ever read: the first sync reads it, or its notice says why it did not.
      return match linked.failure {
        None => page.child(states::reading(ctx)),
        Some(_) => page,
      };
    }
    let search = self.choices.search.get();
    let sort = self.choices.sort.get();
    let rows = linked.rows(tab, &search, sort);
    if rows.is_empty() {
      if linked.count(tab) == 0 {
        return page.child(states::empty_tab(ctx, tab, &linked.repo));
      }
      let typed = self.choices.search.clone();
      return page.child(states::no_matches(ctx, search.trim(), move || typed.set(String::new())));
    }
    let footer = linked.footer(tab, rows.len(), &search);
    let shown = Rows {
      project: id,
      rows: Arc::new(rows),
      now,
      open: actions.open.clone(),
      list_key: format!("pull-requests-rows-{}-{search}-{sort:?}", tab.name()),
    };
    page.child(table::table(ctx, shown)).child(table::footer(ctx, &footer))
  }

  fn notice_actions(&self, ctx: &mut Ctx, project: &ShownProject, actions: &Actions) -> NoticeActions {
    let navigation = Navigation::of(ctx);
    let id = project.open.project.id();
    let idle = !actions.running();
    let run = |action: &work::ListAction| -> Option<Arc<dyn Fn() + Send + Sync>> {
      let (action, root) = (action.clone(), project.open.repo_root.clone());
      idle.then(|| Arc::new(move || action.run(root.clone())) as Arc<dyn Fn() + Send + Sync>)
    };
    NoticeActions {
      check_again: run(&actions.sync),
      use_account: run(&actions.account),
      open_connections: navigation.to(Route::Project(id, ProjectPage::Connections(ConnectionsView::default()))),
    }
  }
}

/// The page inside the content panel (`Content · Pull requests` `X6VN7J`): 28 over it, 40 at its sides, its
/// parts 12 apart.
fn frame() -> Column {
  Column::new()
    .id("screen-pull_requests")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Roomy.role())
        .left(PullRequestsSpacing::Page.role())
        .right(PullRequestsSpacing::Page.role()),
    )
    .spacing(SpacingSize::Md)
}
