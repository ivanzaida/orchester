//! The list part of the screen (`fqAwP`): why the latest sync failed, the search field
//! (`UvDn5`), the linked repository (`rQjxm`), the rows and the footer (`v2SMO`). Its own
//! component, so typing a search re-renders only the list.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  pull_request::{PullRequest, SYNC_LIMIT, SyncedPullRequests},
  time::Timestamp,
};

use super::{
  notice::{NoticeActions, failure_notice},
  row::{PullRequestRow, ROW_HEIGHT, RowProps, column_headings},
};
use crate::{
  messages::Message,
  pull_requests::LinkedInbox,
  theme::ExtraSpacing,
  ui::{
    command_row::Copied,
    field::{ValueTone, styled_input, value_box_sized},
    github_mark::repository_mark,
    icon::{Icon, IconSize, icon},
    message_text,
    notice::Notice,
    panel::RecoveryPanel,
  },
  workspace::OpenProject,
};

/// Search field and sort widths and the height of both (`UvDn5`, `yMfeM`).
const SEARCH_WIDTH: f32 = 360.0;
const SORT_WIDTH: f32 = 164.0;
const CONTROL_HEIGHT: f32 = 38.0;

type Handler = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub struct ListProps {
  pub project: OpenProject,
  pub inbox: LinkedInbox,
  /// Why the latest sync could not run at all.
  pub failed: Option<Message>,
  /// A sync or check runs: Check again and Use this account wait for it.
  pub running: bool,
  pub check_again: Handler,
  pub use_account: Handler,
  pub open_connections: Handler,
  /// Opens a pull request (number, address) in the browser.
  pub open: Arc<dyn Fn(u64, Arc<str>) + Send + Sync>,
  /// Opens a pull request, by number, in Orchester.
  pub show: Arc<dyn Fn(u64) + Send + Sync>,
}

impl PartialEq for ListProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.inbox == other.inbox
      && self.failed == other.failed
      && self.running == other.running
  }
}

impl lurq::app::component::DevtoolsInspectable for ListProps {}

pub struct PullRequestList {
  query: Signal<String>,
  search_focused: Signal<bool>,
  copied: Signal<Copied>,
}

impl Component for PullRequestList {
  type Props = ListProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      query: ctx.signal(String::new()),
      search_focused: ctx.signal(false),
      copied: ctx.signal(Copied::default()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ListProps>().clone();
    let mut column = Column::new()
      .id("pull-requests-list")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .padding(SpacingSize::Section)
      .spacing(ExtraSpacing::Step.role());
    if let Some(error) = &props.failed {
      column = column.child(Notice::new(
        Icon::AlertCircle,
        ctx.t("pullRequests.notice.actionFailed"),
        message_text(ctx, error),
      ));
    }
    let actions = NoticeActions {
      check_again: (!props.running).then(|| props.check_again.clone()),
      use_account: (!props.running).then(|| props.use_account.clone()),
      open_connections: props.open_connections.clone(),
      copied: self.copied.clone(),
    };
    let name = props.project.project.name().as_str();
    column = column.with_children(failure_notice(ctx, &props.inbox, name, Timestamp::now(), &actions));
    match &props.inbox.synced {
      None => column,
      Some(synced) => column
        .child(self.controls(ctx, &props.inbox))
        .with_children(self.rows(ctx, synced, &props)),
    }
  }
}

impl PullRequestList {
  /// Search, the linked repository and the order (`FPrc5`).
  fn controls(&self, ctx: &Ctx, inbox: &LinkedInbox) -> Element {
    let repo = &inbox.repo;
    let repository = Row::new()
      .id("pull-requests-repository")
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(repository_mark(ctx, PaletteColor::TextSecondary))
      .child(
        Text::new(&ctx.t_args(
          "pullRequests.repository",
          [("owner", repo.owner()), ("name", repo.name())],
        ))
        .variant(TypographyStyle::Description)
        .nowrap(),
      );
    // Only the order GitHub's list is read in exists yet.
    let sort = Column::new().width(Dimension::Px(SORT_WIDTH)).child(value_box_sized(
      ctx,
      "pull-requests-sort",
      &ctx.t("pullRequests.sortUpdated"),
      ValueTone::Unavailable,
      None,
      Dimension::Px(CONTROL_HEIGHT),
    ));
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .child(self.search(ctx))
      .child(Spacer::new().flex(1.0))
      .child(repository)
      .child(sort)
      .into()
  }

  /// The search field (`UvDn5`): the one control of the screen that takes focus.
  fn search(&self, ctx: &Ctx) -> Element {
    let input = ctx.theme().form().input.clone();
    let border = if self.search_focused.get() {
      Border::inside(BorderSize::Md, input.border_focus.clone())
    } else {
      Border::inside(BorderSize::Sm, input.border.clone())
    };
    let (focus, blur) = (self.search_focused.clone(), self.search_focused.clone());
    let field = styled_input(ctx, self.query.clone(), TypographyStyle::FieldLabel)
      .id("pull-requests-search")
      .single_line()
      .flex(1.0)
      .placeholder(&ctx.t("pullRequests.search"))
      .on_focus(move || focus.set(true))
      .on_blur(move || blur.set(false));
    Row::new()
      .width(Dimension::Px(SEARCH_WIDTH))
      .height(Dimension::Px(CONTROL_HEIGHT))
      .padding_custom(input.padding)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(input.background)
      .border(border)
      .rounded(input.radius)
      .child(field)
      .child(icon(Icon::Search, IconSize::Dismiss, PaletteColor::TextSecondary))
      .into()
  }

  /// Headings, rows and the footer (`Qcgjy`, `j27hk2`, `v2SMO`), 20px apart. The rows scroll only
  /// when they do not fit; otherwise the footer follows the last row. Nothing open: the empty
  /// panel, centred like 08.D.
  fn rows(&self, ctx: &mut Ctx, synced: &SyncedPullRequests, props: &ListProps) -> Vec<Element> {
    let total = synced.pull_requests.len();
    if total == 0 {
      let repo = &props.inbox.repo;
      let panel = RecoveryPanel::new(
        Icon::GitPullRequest,
        &ctx.t("pullRequests.empty.title"),
        &ctx.t_args(
          "pullRequests.empty.detail",
          [("owner", repo.owner()), ("name", repo.name())],
        ),
      );
      let empty = Column::new()
        .id("pull-requests-empty")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .align_items(Alignment::Center)
        .justify(Justify::Center)
        .child(panel);
      return vec![empty.into()];
    }
    let query = self.query.get();
    let (open, show) = (props.open.clone(), props.show.clone());
    let rows: Vec<Element> = synced
      .pull_requests
      .iter()
      .filter(|pull_request| pull_request.matches(&query))
      .map(|pull_request| {
        ctx.mount_keyed::<PullRequestRow>(
          &pull_request.number().to_string(),
          row_props(pull_request, &open, &show),
        )
      })
      .collect();
    let shown = rows.len();
    let table: Element = if rows.is_empty() {
      Text::new(&ctx.t_args("pullRequests.footer.noMatch", [("query", query.trim())]))
        .variant(TypographyStyle::Caption)
        .into()
    } else {
      // At most `SYNC_LIMIT` rows of a fixed height: laid out in full, drawn only where visible.
      ScrollVertical::new(
        Column::new()
          .id("pull-requests-row-stack")
          .width(Dimension::Pct(100.0))
          .with_children(rows),
      )
        .id("pull-requests-rows")
        .culling(true)
        .width(Dimension::Pct(100.0))
        // Takes the free height, but never more than the rows need, so the footer follows the
        // last row when they fit and the rows scroll when they do not.
        .flex(1.0)
        .max_height(Dimension::Px(ROW_HEIGHT * shown as f32))
        .into()
    };
    vec![
      column_headings(ctx),
      table,
      footer(ctx, shown, total, !query.trim().is_empty()),
    ]
  }
}

fn row_props(
  pull_request: &PullRequest,
  open: &Arc<dyn Fn(u64, Arc<str>) + Send + Sync>,
  show: &Arc<dyn Fn(u64) + Send + Sync>,
) -> RowProps {
  let (open, show, number, url) = (
    open.clone(),
    show.clone(),
    pull_request.number(),
    Arc::<str>::from(pull_request.url()),
  );
  RowProps {
    pull_request: pull_request.clone(),
    open: Arc::new(move || open(number, url.clone())),
    show: Arc::new(move || show(number)),
  }
}

/// How many are open (of them, how many the search shows), and where they come from (`FAPiA`,
/// `sHUhq`).
fn footer(ctx: &Ctx, shown: usize, total: usize, searching: bool) -> Element {
  let (shown_text, total_text) = (shown.to_string(), total.to_string());
  let text = if searching {
    ctx.t_args(
      "pullRequests.footer.filtered",
      [("shown", shown_text.as_str()), ("count", total_text.as_str())],
    )
  } else if total >= SYNC_LIMIT as usize {
    ctx.t_args("pullRequests.footer.limited", [("count", total_text.as_str())])
  } else if total == 1 {
    ctx.t("pullRequests.footer.one")
  } else {
    ctx.t_args("pullRequests.footer.other", [("count", total_text.as_str())])
  };
  Row::new()
    .id("pull-requests-footer")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .child(Text::new(&text).variant(TypographyStyle::Caption))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&ctx.t("pullRequests.footer.scope")).variant(TypographyStyle::Caption))
    .into()
}
