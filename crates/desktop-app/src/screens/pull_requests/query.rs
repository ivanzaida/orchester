//! The tabs (`Tabs` `vHiQc`) with how many pull requests each holds, `—` until the list is known, and what
//! narrows and orders the rows (`Query line` `rOxhG`): the search, the sort and when syncs run. The tab is
//! part of the page's path (`?tab=`); the search and the sort belong to the open page.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, SelectOption, Spacer, Stack, Text},
  core::Signal,
  layout::Alignment,
  node::dimension::Dimension,
};
use orchester_core::id::ProjectId;
use orchester_workspace::pull_requests::list::{LinkedList, PullRequestSort, PullRequestTab};

use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    field::{FieldInput, SelectField},
    icon::{Icon, icon},
    tabs::{PageTab, counted, page_tabs},
  },
};

/// `Search` `J2JGV` and `Sort` `E0lNS`.
const SEARCH_WIDTH: f32 = 300.0;
const SORT_WIDTH: f32 = 180.0;

/// What the person chose to narrow and order the rows.
#[derive(Clone)]
pub(super) struct Choices {
  pub search: Signal<String>,
  pub sort: Signal<PullRequestSort>,
}

/// The tabs, `tab` the current one, each with its count once `list` is known.
pub(super) fn tabs(ctx: &mut Ctx, project: ProjectId, tab: PullRequestTab, list: Option<&LinkedList>) -> Stack {
  let navigation = Navigation::of(ctx);
  let unknown = ctx.t("pull_requests.tabs.unknownCount");
  let tabs: Vec<PageTab> = PullRequestTab::ALL
    .into_iter()
    .map(|shown| {
      let word = ctx.t(label_key(shown));
      let label = match list {
        Some(list) => counted(ctx, &word, list.count(shown)),
        None => ctx.t_args("ui.tabCount", [("label", word), ("count", unknown.clone())]),
      };
      PageTab::to(
        format!("pull-requests-tab-{}", shown.name()),
        label,
        shown == tab,
        navigation.to(Route::Project(project, ProjectPage::PullRequests(shown))),
      )
    })
    .collect();
  page_tabs(ctx, "pull-requests-tabs", tabs)
}

pub(super) const fn label_key(tab: PullRequestTab) -> &'static str {
  match tab {
    PullRequestTab::Open => "pull_requests.tabs.open",
    PullRequestTab::NeedsReview => "pull_requests.tabs.needsReview",
    PullRequestTab::AgentWorking => "pull_requests.tabs.agentWorking",
    PullRequestTab::Merged => "pull_requests.tabs.merged",
    PullRequestTab::Closed => "pull_requests.tabs.closed",
  }
}

/// The search, the sort, and when syncs run.
pub(super) fn query(ctx: &mut Ctx, choices: &Choices) -> Row {
  let search = FieldInput::new(
    "pull-requests-search",
    ctx.t("pull_requests.query.search"),
    choices.search.clone(),
  )
  .search()
  .clearable(ctx.t("pull_requests.query.clearSearch"))
  .placeholder(ctx.t("pull_requests.query.searchPlaceholder"))
  .width(Dimension::Px(SEARCH_WIDTH))
  .render(ctx);
  let sorts = [
    (PullRequestSort::RecentlyUpdated, "pull_requests.sort.recentlyUpdated"),
    (
      PullRequestSort::LeastRecentlyUpdated,
      "pull_requests.sort.leastRecentlyUpdated",
    ),
    (PullRequestSort::Number, "pull_requests.sort.number"),
  ]
  .map(|(sort, key)| SelectOption::new(sort, ctx.t(key)))
  .to_vec();
  let sort = SelectField::new(
    "pull-requests-sort",
    ctx.t("pull_requests.query.sort"),
    choices.sort.clone(),
    sorts,
  )
  .width(Dimension::Px(SORT_WIDTH))
  .render(ctx);
  Row::new()
    .id("pull-requests-query")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(search)
    .child(sort)
    .child(Spacer::new().flex(1.0))
    .child(
      Row::new()
        .id("pull-requests-sync-note")
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Info, IconSize::Base, PaletteColor::TextMuted))
        .child(Text::new(&ctx.t("pull_requests.query.syncNote")).variant(ExtraTypography::Note.role())),
    )
}
