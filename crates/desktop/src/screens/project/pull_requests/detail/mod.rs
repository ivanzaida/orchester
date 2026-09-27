//! A pull request (08.02 `Rrmhf`), read-only: the topbar with the way back to the inbox and Open
//! in GitHub (`PGv7D`, `iwbRq`), the heading and context line (`y4WZMq`, `ADELW`), the review
//! tabs (`mwzXw`), the changed files (`h8PZJa`) and the selected file's diff (`rz27k`). The pull
//! request is read when the screen opens and on Refresh; the diff is cached per head revision, so
//! an unchanged head is not fetched again. What was read last shows first, and a failed refresh
//! keeps it under a notice.
//!
//! Agent feedback (the feedback panel `q0Dz8e`, line selection `i1dEXj`, agent verification
//! `Cttad`, the linked task button `Y0cC2`) comes with runs and is left out; its tabs are shown
//! unavailable. Only Find a file takes focus (`docs/design/orchester-workspace.md`, "Focus").

mod actions;
mod diff_pane;
mod heading;
mod state;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::{revision::Revision, time::Timestamp};
use parking_lot::Mutex;

use self::{
  actions::Actions,
  diff_pane::{DiffPane, DiffPaneProps},
  heading::{HeadingProps, PullRequestHeading},
};
use super::{
  super::{ProjectNav, page},
  view::SyncLine,
};
use crate::{
  messages::Message,
  navigation::route::Page,
  pull_requests::detail::{Detail, DiffState, LinkedDetail, ReadDiff},
  theme::ExtraSpacing,
  ui::{
    button::ActionButton,
    diff::{FileEntry, FileGroup, FileList, FileListProps},
    icon::Icon,
    message_text,
    notice::Notice,
    panel::RecoveryPanel,
    pressable::{pressable, surface},
    tab::{Tab, tab_row},
  },
  workspace::OpenProject,
};

/// Review tab row and its tabs (`mwzXw`, `XKO6k`).
const TAB_ROW_HEIGHT: f32 = 42.0;
const TAB_HEIGHT: f32 = 40.0;

/// The file list of the diff of one head.
type Files = Option<(Revision, Arc<Vec<FileGroup>>)>;

#[derive(Clone, Debug, PartialEq)]
pub struct PullRequestProps {
  pub project: OpenProject,
  pub number: u64,
}

impl lurq::app::component::DevtoolsInspectable for PullRequestProps {}

pub struct PullRequestScreen {
  /// What the latest refresh returned; before it, what is stored is shown.
  latest: Signal<Option<Detail>>,
  /// Why the latest refresh could not run at all (storage, a stopped worker).
  failed: Signal<Option<Message>>,
  /// Position of the selected file in the diff.
  selected: Signal<usize>,
  /// The file list of the diff on screen, kept per head so the list sees the same entries.
  files: Arc<Mutex<Files>>,
  nav: ProjectNav,
}

impl Component for PullRequestScreen {
  type Props = PullRequestProps;

  fn create(ctx: &mut Ctx) -> Self {
    let screen = Self {
      latest: ctx.signal(None),
      failed: ctx.signal(None),
      selected: ctx.signal(0),
      files: Arc::new(Mutex::new(None)),
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    };
    let props = ctx.props::<PullRequestProps>().clone();
    let actions = Actions::of(ctx, &props.project, props.number);
    actions.watch(ctx, &screen.latest, &screen.failed);
    actions.refresh.run(());
    screen
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PullRequestProps>().clone();
    // Futures are matched by call order: the actions come first, as in `create`.
    let actions = Actions::of(ctx, &props.project, props.number);
    let stored = actions::load(ctx, &props.project, props.number);
    let loaded = stored.state().get();
    let detail = self.latest.get().or(loaded.data);
    let running = actions.running();
    let mut column = Column::new()
      .id("pull-request")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(self.topbar(ctx, &props, detail.as_ref(), &actions));
    let body = match &detail {
      None => match loaded.error {
        Some(error) => centered(RecoveryPanel::new(
          Icon::AlertCircle,
          &ctx.t("pullRequests.detail.loadFailed"),
          &message_text(ctx, &error),
        )),
        None => centered(state::loading_panel(ctx)),
      },
      Some(Detail::Unlinked) => self.unlinked(ctx, &props),
      Some(Detail::Linked(linked)) => {
        column = column.child(heading(ctx, linked, running)).child(tabs(ctx, linked));
        if let Some(error) = self.failed.get() {
          column = column.child(inset(Notice::new(
            Icon::AlertCircle,
            ctx.t("pullRequests.detail.actionFailed"),
            message_text(ctx, &error),
          )));
        }
        self.review(ctx, linked, running)
      }
    };
    column.child(body)
  }
}

impl PullRequestScreen {
  /// The breadcrumb back to the inbox, Refresh and Open in GitHub (`PGv7D`). Refresh is off
  /// while a refresh runs and until a rate limit resets.
  fn topbar(&self, ctx: &Ctx, props: &PullRequestProps, detail: Option<&Detail>, actions: &Actions) -> Element {
    let number = props.number.to_string();
    let nav = self.nav.clone();
    let breadcrumb = pressable(surface().id("pull-request-breadcrumb"), move || {
      nav.show(Page::PullRequests);
    })
    .child(
      Text::new(&ctx.t_args(
        "pullRequests.detail.breadcrumb",
        [
          ("project", props.project.project.name().as_str()),
          ("number", number.as_str()),
        ],
      ))
      .variant(TypographyStyle::Description)
      .nowrap(),
    );
    let linked = match detail {
      Some(Detail::Linked(linked)) => Some(linked),
      Some(Detail::Unlinked) | None => None,
    };
    let refresh_action = actions.refresh.clone();
    let refresh = ActionButton::new(ctx.t("pullRequests.refresh"))
      .icon(Icon::RefreshCw)
      .id("pull-request-refresh")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
      .on_click(move || refresh_action.run(()))
      .enabled(linked.is_some() && actions.may_refresh(detail, Timestamp::now()));
    let open = ActionButton::new(ctx.t("pullRequests.row.openInGitHub"))
      .icon(Icon::ExternalLink)
      .id("pull-request-open")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT));
    let open = match linked.and_then(|linked| linked.pull_request.as_ref()) {
      Some(pull_request) => {
        let (browse, number, url) = (actions.open.clone(), props.number, Arc::<str>::from(pull_request.url()));
        open.on_click(move || browse.run((number, url.clone())))
      }
      None => open.on_click(|| {}).enabled(false),
    };
    page::topbar_with(breadcrumb.into(), [refresh.render(ctx), open.render(ctx)])
  }

  /// The project is not linked to a GitHub repository any more (as 08.D).
  fn unlinked(&self, ctx: &Ctx, props: &PullRequestProps) -> Element {
    let nav = self.nav.clone();
    let action = ActionButton::new(ctx.t("pullRequests.unavailable.openConnections"))
      .icon(Icon::Plug)
      .id("pull-request-open-connections")
      .on_click(move || nav.show(Page::CONNECTIONS))
      .render(ctx);
    centered(
      RecoveryPanel::new(
        Icon::GitPullRequest,
        &ctx.t("pullRequests.unavailable.title"),
        &ctx.t_args(
          "pullRequests.unavailable.detail",
          [("project", props.project.project.name().as_str())],
        ),
      )
      .action(action),
    )
  }

  /// The failure notice, then the files and the diff (`ZnI9n`), or what stands in for them.
  fn review(&self, ctx: &mut Ctx, linked: &LinkedDetail, running: bool) -> Element {
    let now = Timestamp::now();
    let read = match &linked.diff {
      DiffState::Read(read) => read,
      DiffState::TooLarge => {
        return with_notice(
          ctx,
          linked,
          centered(state::too_large_panel(ctx, linked.pull_request.as_ref())),
        );
      }
      DiffState::Unknown => {
        let panel = match linked.latest_failure() {
          Some(failure) if !running => state::failure_panel(ctx, linked, failure, now),
          _ => state::loading_panel(ctx),
        };
        return centered(panel);
      }
    };
    if read.diff.files().is_empty() {
      return with_notice(ctx, linked, centered(state::empty_panel(ctx)));
    }
    let files = self.files(ctx, read);
    let index = self.selected.get().min(read.diff.files().len() - 1);
    let list = ctx.mount_keyed::<FileList>(
      "pull-request-files",
      FileListProps {
        groups: files,
        selected: index,
        select: self.selected.clone(),
      },
    );
    let pane = ctx.mount_keyed::<DiffPane>(
      "pull-request-diff",
      DiffPaneProps {
        diff: read.clone(),
        index,
        base_branch: linked
          .pull_request
          .as_ref()
          .map(|pull_request| pull_request.base().as_str().into()),
      },
    );
    let workspace = Row::new()
      .id("pull-request-workspace")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(list)
      .child(pane);
    with_notice(ctx, linked, workspace.into())
  }

  fn files(&self, ctx: &Ctx, read: &ReadDiff) -> Arc<Vec<FileGroup>> {
    let mut files = self.files.lock();
    if let Some((head, groups)) = files.as_ref()
      && *head == read.head
    {
      return groups.clone();
    }
    let count = read.diff.files().len().to_string();
    let groups = Arc::new(vec![FileGroup {
      key: "diff-file",
      heading: ctx.t_args("diff.files.heading", [("count", count.as_str())]),
      files: read
        .diff
        .files()
        .iter()
        .enumerate()
        .map(|(index, file)| FileEntry::of(index, file))
        .collect(),
    }]);
    *files = Some((read.head.clone(), groups.clone()));
    groups
  }
}

/// The heading and context line (`y4WZMq`).
fn heading(ctx: &mut Ctx, linked: &LinkedDetail, running: bool) -> Element {
  let line = if running {
    SyncLine::Syncing
  } else {
    SyncLine::after_read(linked.synced_at, linked.failure.as_ref())
  };
  let head = match &linked.diff {
    DiffState::Read(read) => Some(read.head.clone()),
    DiffState::Unknown | DiffState::TooLarge => None,
  };
  ctx.mount_keyed::<PullRequestHeading>(
    "pull-request-heading",
    HeadingProps {
      number: linked.number,
      pull_request: linked.pull_request.clone(),
      head,
      line,
    },
  )
}

/// Files changed, and the tabs that need agent runs, which do not exist yet (`mwzXw`).
fn tabs(ctx: &Ctx, linked: &LinkedDetail) -> Element {
  let height = Dimension::Px(TAB_HEIGHT);
  let files = match &linked.diff {
    DiffState::Read(read) => Some(read.diff.files().len()),
    // Not read (too large, or the read failed), but GitHub said how many files it changes.
    DiffState::TooLarge | DiffState::Unknown => linked
      .pull_request
      .as_ref()
      .map(|pull_request| usize::try_from(pull_request.changes().files).expect("a u32 fits in usize on 32 and 64 bit")),
  };
  let tab = |id, key| Tab::new(id, ctx.t(key), height);
  tab_row(
    Dimension::Px(TAB_ROW_HEIGHT),
    ExtraSpacing::Group.role(),
    Some(SpacingSize::Lg),
    [
      tab("pull-request-tab-files", "pullRequests.detail.tabs.files")
        .count(files)
        .selected(true),
      tab("pull-request-tab-feedback", "pullRequests.detail.tabs.feedback"),
      tab("pull-request-tab-checks", "pullRequests.detail.tabs.checks"),
      tab("pull-request-tab-overview", "pullRequests.detail.tabs.overview"),
    ],
  )
}

/// `content` under the notice of a failed refresh, if there is one.
fn with_notice(ctx: &Ctx, linked: &LinkedDetail, content: Element) -> Element {
  let mut column = Column::new().width(Dimension::Pct(100.0)).flex(1.0);
  if let Some(notice) = state::failure_notice(ctx, linked, Timestamp::now()) {
    column = column.child(inset(notice));
  }
  column.child(content).into()
}

fn inset(content: impl Into<Element>) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Lg)
    .padding_bottom(SpacingSize::Md)
    .child(content)
    .into()
}

fn centered(content: impl Into<Element>) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(SpacingSize::Section)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
    .into()
}
