//! Pull requests (08.01 `wGtxT`): the open pull requests of the project's linked GitHub
//! repository, read-only. Sync follows `docs/design/orchester-workspace.md`, "GitHub", rule 9:
//! when the screen opens, on Refresh, and every 60 s while it is shown; the timer lives with the
//! screen, so nothing polls once another screen replaces it. A failed sync keeps the last list
//! under a notice (08.E–08.H, see `notice`); without a linked repository the screen is
//! unavailable (08.D `dmib5`). Only the search field takes focus.

mod actions;
mod detail;
mod list;
mod notice;
mod row;
mod sync_status;
mod view;

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::Column,
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::time::Timestamp;

pub use self::detail::{PullRequestProps, PullRequestScreen};
use self::{
  actions::{Actions, InboxAction},
  list::ListProps,
  sync_status::{SyncStatus, SyncStatusProps},
  view::SyncLine,
};
use super::{ProjectNav, page};
use crate::{
  messages::Message,
  navigation::route::Page,
  pull_requests::{Inbox, LinkedInbox},
  theme::ExtraSpacing,
  ui::{
    button::ActionButton,
    icon::Icon,
    message_text,
    panel::RecoveryPanel,
    tab::{Tab, tab_row},
  },
  workspace::OpenProject,
};

/// Automatic sync while the screen is shown (design rule 9).
const SYNC_INTERVAL: Duration = Duration::from_secs(60);
/// Tab row (`DF8II`).
const TAB_ROW_HEIGHT: f32 = 42.0;
/// Width of the unavailable panel (`MBmo4` in 08.D), which the empty inbox, without a design
/// node of its own, shares.
const PANEL_WIDTH: f32 = 520.0;

pub struct PullRequestsScreen {
  /// What the latest sync or *Use this account* returned; before it, the stored list is shown.
  latest: Signal<Option<Inbox>>,
  /// Why the latest sync could not run at all (storage, a stopped worker).
  failed: Signal<Option<Message>>,
  nav: ProjectNav,
}

impl Component for PullRequestsScreen {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    let screen = Self {
      latest: ctx.signal(None),
      failed: ctx.signal(None),
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    };
    let open = ctx.props::<OpenProject>().clone();
    let actions = Actions::of(ctx, &open);
    actions.watch(ctx, &screen.latest, &screen.failed);
    actions.sync.run(open.repo_root.clone());
    let (latest, repo_root) = (screen.latest.clone(), open.repo_root);
    ctx
      .create_interval(SYNC_INTERVAL, move || {
        if actions.may_sync(latest.get_untracked().as_ref(), Timestamp::now()) {
          actions.sync.run(repo_root.clone());
        }
      })
      .start();
    screen
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<OpenProject>().clone();
    // Futures are matched by call order: the actions come first, as in `create`.
    let actions = Actions::of(ctx, &open);
    let stored = actions::load(ctx, &open);
    let loaded = stored.state().get();
    let inbox = self.latest.get().or(loaded.data);
    let line = SyncLine::of(inbox.as_ref(), actions.running());
    let body = match &inbox {
      None => match loaded.error {
        Some(error) => centered(
          RecoveryPanel::new(
            Icon::AlertCircle,
            &ctx.t("pullRequests.loadFailed"),
            &message_text(ctx, &error),
          )
          .into(),
        ),
        None => Column::new().flex(1.0).into(),
      },
      Some(Inbox::Unlinked) => self.unavailable(ctx, &open),
      Some(Inbox::Linked(linked)) => {
        let props = self.list_props(&open, linked, &actions);
        ctx.mount_keyed::<list::PullRequestList>("pull-request-list", props)
      }
    };
    let count = match &inbox {
      Some(Inbox::Linked(linked)) => linked.synced.as_ref().map(|synced| synced.pull_requests.len()),
      _ => None,
    };
    let breadcrumb = ctx.t_args("pullRequests.breadcrumb", [("project", open.project.name().as_str())]);
    let refresh = self.refresh(ctx, &actions, inbox.as_ref(), &open);
    let status = ctx.mount_keyed::<SyncStatus>("sync-status", SyncStatusProps { line });
    // Without a linked repository there is nothing to filter: 08.D has no tab row.
    let tabs = (!matches!(inbox, Some(Inbox::Unlinked))).then(|| tabs(ctx, count));
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(page::topbar(&breadcrumb, [status, refresh]))
      .child(page::screen_heading(
        &ctx.t("pullRequests.title"),
        &ctx.t("pullRequests.description"),
      ))
      .with_children(tabs)
      .child(body)
  }
}

impl PullRequestsScreen {
  fn list_props(&self, open: &OpenProject, inbox: &LinkedInbox, actions: &Actions) -> ListProps {
    let run = |action: &InboxAction| -> Handler {
      let (action, repo_root) = (action.clone(), open.repo_root.clone());
      Arc::new(move || action.run(repo_root.clone()))
    };
    let (browse, show) = (actions.open.clone(), self.nav.clone());
    ListProps {
      project: open.clone(),
      inbox: inbox.clone(),
      failed: self.failed.get(),
      running: actions.running(),
      check_again: run(&actions.sync),
      use_account: run(&actions.account),
      open_connections: self.nav.link(Page::CONNECTIONS),
      open: Arc::new(move |number, url| browse.run((number, url))),
      show: Arc::new(move |number| show.link(Page::PullRequest(number))),
    }
  }

  /// Refresh (`x5LlA`): off without a linked repository, while a sync runs and until a rate
  /// limit resets.
  fn refresh(&self, ctx: &Ctx, actions: &Actions, inbox: Option<&Inbox>, open: &OpenProject) -> Element {
    let linked = matches!(inbox, Some(Inbox::Linked(_)));
    let (sync, repo_root) = (actions.sync.clone(), open.repo_root.clone());
    ActionButton::new(ctx.t("pullRequests.refresh"))
      .icon(Icon::RefreshCw)
      .id("pull-requests-refresh")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
      .on_click(move || sync.run(repo_root.clone()))
      .enabled(linked && actions.may_sync(inbox, Timestamp::now()))
      .render(ctx)
  }

  /// No linked repository (08.D `dmib5`).
  fn unavailable(&self, ctx: &Ctx, open: &OpenProject) -> Element {
    let action = ActionButton::new(ctx.t("pullRequests.unavailable.openConnections"))
      .icon(Icon::Plug)
      .id("pull-requests-open-connections")
      .link(self.nav.link(Page::CONNECTIONS))
      .render(ctx);
    let panel = Column::new().id("pull-requests-unavailable").child(
      RecoveryPanel::new(
        Icon::GitPullRequest,
        &ctx.t("pullRequests.unavailable.title"),
        &ctx.t_args(
          "pullRequests.unavailable.detail",
          [("project", open.project.name().as_str())],
        ),
      )
      .width(PANEL_WIDTH)
      .action(action),
    );
    centered(panel.into())
  }
}

/// Open, and the tabs that need agent runs or closed pull requests, which are not read yet
/// (`DF8II`).
fn tabs(ctx: &Ctx, open_count: Option<usize>) -> Element {
  let height = Dimension::Px(TAB_ROW_HEIGHT);
  let tab = |id, key| Tab::new(id, ctx.t(key), height);
  tab_row(
    height,
    SpacingSize::Md,
    Some(SpacingSize::Section),
    [
      tab("pull-requests-tab-open", "pullRequests.tabs.open")
        .count(open_count)
        .selected(true),
      tab("pull-requests-tab-needs-review", "pullRequests.tabs.needsReview"),
      tab("pull-requests-tab-agent-working", "pullRequests.tabs.agentWorking"),
      tab("pull-requests-tab-merged", "pullRequests.tabs.merged"),
      tab("pull-requests-tab-closed", "pullRequests.tabs.closed"),
    ],
  )
}

fn centered(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(SpacingSize::Section)
    .spacing(ExtraSpacing::Step.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
    .into()
}

type Handler = Arc<dyn Fn() + Send + Sync>;
