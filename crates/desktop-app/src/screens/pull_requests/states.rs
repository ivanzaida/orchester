//! What the page shows instead of rows. Without a linked repository pull requests are unavailable, with
//! Open Connections (`State · Pull requests are unavailable` `AmLs6` in 08.D, 520 wide, 64 under the header);
//! while the first sync reads them, the table's header stands over the loader; when the stored list cannot be
//! read, why, with Try again; a tab without pull requests says so under the header (`Empty` `r1gusH`); and a
//! search that hides every row says so, with Clear. Loading and a failure never look like an empty list.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, repository::GitHubRepo};
use orchester_workspace::{messages::Message, pull_requests::list::PullRequestTab};

use super::table;
use crate::{
  navigation::Navigation,
  routes::{ConnectionsView, ProjectPage, Route},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, PullRequestsSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    state::{EmptyState, load_error, loading},
  },
};

/// `State · Pull requests are unavailable` and the glyph tile of an empty tab (`k3euSu`).
const UNAVAILABLE_WIDTH: f32 = 520.0;
const GLYPH_TILE: f32 = 36.0;

/// The project has no linked GitHub repository: Open Connections.
pub(super) fn unavailable(ctx: &mut Ctx, project: ProjectId, name: &str) -> Element {
  let navigation = Navigation::of(ctx);
  let open = ActionButton::new(
    "pull-requests-open-connections",
    ButtonKind::Primary,
    ctx.t("pull_requests.unavailable.openConnections"),
  )
  .icon(Icon::Plug)
  .on_click(move || {
    navigation.go(Route::Project(
      project,
      ProjectPage::Connections(ConnectionsView::default()),
    ));
  });
  let panel = EmptyState::new(
    "pull-requests-unavailable",
    Icon::Unplug,
    ctx.t("pull_requests.unavailable.title"),
    ctx.t_args("pull_requests.unavailable.detail", [("project", name)]),
  )
  .action(open);
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(PullRequestsSpacing::Unavailable.role()))
    .align_items(Alignment::Center)
    .child(Column::new().width(Dimension::Px(UNAVAILABLE_WIDTH)).child(panel))
    .into()
}

/// The table's header over the loader, while the first sync reads the pull requests.
pub(super) fn reading(ctx: &Ctx) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(table::header(ctx))
    .child(loading("pull-requests-loading", &ctx.t("pull_requests.reading")))
}

/// The stored list cannot be read: why, and Try again.
pub(super) fn failed(ctx: &Ctx, error: &Message, retry: impl Fn() + Send + Sync + 'static) -> Element {
  let body = ctx.t_args("pull_requests.loadFailed.body", [("reason", message_text(ctx, error))]);
  load_error(
    ctx,
    "pull-requests-load-error",
    (&ctx.t("pull_requests.loadFailed.title"), &body),
    retry,
  )
}

/// `tab` holds no pull request of `repo`.
pub(super) fn empty_tab(ctx: &Ctx, tab: PullRequestTab, repo: &GitHubRepo) -> Column {
  let (glyph, title, detail) = match tab {
    PullRequestTab::Open => (
      Icon::GitPullRequest,
      "pull_requests.empty.open.title",
      "pull_requests.empty.open.detail",
    ),
    PullRequestTab::NeedsReview => (
      Icon::Eye,
      "pull_requests.empty.needsReview.title",
      "pull_requests.empty.needsReview.detail",
    ),
    PullRequestTab::AgentWorking => (
      Icon::Bot,
      "pull_requests.empty.agentWorking.title",
      "pull_requests.empty.agentWorking.detail",
    ),
    PullRequestTab::Merged => (
      Icon::GitMerge,
      "pull_requests.empty.merged.title",
      "pull_requests.empty.merged.detail",
    ),
    PullRequestTab::Closed => (
      Icon::GitPullRequestClosed,
      "pull_requests.empty.closed.title",
      "pull_requests.empty.closed.detail",
    ),
  };
  let detail = ctx.t_args(detail, [("owner", repo.owner()), ("name", repo.name())]);
  no_rows(ctx, (glyph, &ctx.t(title), &detail), None)
}

/// The search hides every one of the tab's rows; Clear empties it.
pub(super) fn no_matches(ctx: &Ctx, query: &str, clear: impl Fn() + Send + Sync + 'static) -> Column {
  let detail = ctx.t_args("pull_requests.noMatches.detail", [("query", query)]);
  let clear = ActionButton::new(
    "pull-requests-clear-search",
    ButtonKind::Secondary,
    ctx.t("pull_requests.noMatches.clear"),
  )
  .icon(Icon::X)
  .on_click(clear);
  no_rows(
    ctx,
    (Icon::SearchX, &ctx.t("pull_requests.noMatches.title"), &detail),
    Some(clear),
  )
}

/// The table's header over the glyph, why there is no row, and what can be done about it.
fn no_rows(ctx: &Ctx, (glyph, title, detail): (Icon, &str, &str), action: Option<ActionButton>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(table::header(ctx))
    .child(
      Column::new()
        .id("pull-requests-no-rows")
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().top(PullRequestsSpacing::Empty.role()))
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Center)
        .child(tile(glyph))
        .child(
          Text::new(title)
            .id("pull-requests-no-rows-title")
            .variant(ExtraTypography::Strong.role()),
        )
        .child(
          Text::new(detail)
            .id("pull-requests-no-rows-detail")
            .variant(TypographyStyle::Description),
        )
        .with_children(action.map(|action| Row::new().child(action))),
    )
}

/// The glyph on its tile.
fn tile(glyph: Icon) -> Row {
  Row::new()
    .width(Dimension::Px(GLYPH_TILE))
    .height(Dimension::Px(GLYPH_TILE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::Tile.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Xl, PaletteColor::TextSecondary))
}
