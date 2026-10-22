//! The top of a pull request's page (`PR header` `nQXR1`): the crumbs, the title with its number, Refresh
//! and Open in GitHub; the organization row with the state, the review, the branches, the task, the checks and,
//! at its end, the head and how its latest read went; and the page's tabs (`Page tabs` `Vlhg7` in `S5G87Z`), Files
//! changed with its count, Agent feedback with its open threads, and the planned ones locked, why at the row's end.
//! While nothing is known yet the title is the pull request's number and the status says it is read (`Reading` in
//! `c07NAC`).

use std::{num::NonZeroU64, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{pull_request::PullRequest, time::Timestamp};
use orchester_workspace::{
  display,
  pull_requests::{list::PullRequestTab, page::PullRequestView, status::SyncStatus},
};

use super::super::{
  labels,
  status::{Shown, parts, status_line},
};
use crate::{
  navigation::{Navigation, link::Open},
  project::ShownProject,
  routes::{OverviewTab, ProjectPage, Route, TaskTab, TaskView},
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, IconSize, PullRequestsSpacing, PullRequestsText},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::opening,
    tabs::{PageTab, counted, page_tabs},
    tooltip::Truncated,
    unavailable::reason_line,
  },
};

/// What the header's buttons do; `None` while they cannot: Refresh while a read runs, Open in GitHub
/// before the pull request's address is known.
pub(super) struct Actions {
  pub refresh: Option<Arc<dyn Fn() + Send + Sync>>,
  pub open: Option<Arc<dyn Fn() + Send + Sync>>,
}

/// The page's own tabs: which shows, and how many feedback threads are open once read.
pub(super) struct Tabs {
  pub feedback_shown: bool,
  pub feedback_count: Option<usize>,
}

/// The organization row's height (`Organization` `pp4Q0`), a separator's (`Sep` `uKnbH`), the gap under the
/// organization row besides the header's own (`Gap` in `nQXR1`) and the tab row's height (`Page tabs` `UMBLT`).
const ORGANIZATION_HEIGHT: f32 = 28.0;
const ORGANIZATION_GAP: f32 = 2.0;
const SEPARATOR_HEIGHT: f32 = 14.0;
const TABS_HEIGHT: f32 = 34.0;

/// The header of a pull request nothing is known of yet.
pub(super) fn compact(ctx: &mut Ctx, (project, number): (&ShownProject, u64), reading: bool, shown: &Tabs) -> Column {
  let title = Text::new(&ctx.t_args("pull_requests.detail.title", [("number", number.to_string())]))
    .id("pull-request-title")
    .variant(TypographyStyle::Heading);
  let status = if reading {
    Shown::Syncing
  } else {
    Shown::After(SyncStatus::NotSynced)
  };
  let status = status_line(ctx, &status, Timestamp::now());
  frame()
    .child(crumbs(ctx, project, number))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .align_items(Alignment::Center)
        .child(title)
        .child(Row::new().flex(1.0))
        .child(status),
    )
    .child(tabs(ctx, (project, number), (None, shown)))
}

pub(super) fn header(
  ctx: &mut Ctx,
  (project, view): (&ShownProject, &PullRequestView),
  actions: &Actions,
  reading: bool,
  shown: &Tabs,
) -> Column {
  let Some(pull_request) = view.pull_request.as_ref() else {
    return compact(ctx, (project, view.number), reading, shown);
  };
  let organization = organization(ctx, project, view, pull_request, reading);
  frame()
    .child(crumbs(ctx, project, view.number))
    .child(title_row(ctx, pull_request, actions))
    .child(organization)
    .child(Row::new().height(Dimension::Px(ORGANIZATION_GAP)))
    .child(tabs(ctx, (project, view.number), (view.file_count(), shown)))
}

fn frame() -> Column {
  Column::new()
    .id("pull-request-header")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Lg)
        .left(PullRequestsSpacing::Detail.role())
        .right(PullRequestsSpacing::Detail.role()),
    )
    .spacing(SpacingSize::Sm)
}

/// Kontur / Pull requests / #142.
fn crumbs(ctx: &mut Ctx, project: &ShownProject, number: u64) -> Element {
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  breadcrumb(
    ctx,
    "pull-request-crumbs",
    CrumbSeparator::Slash,
    vec![
      Crumb::to(
        project.open.project.name().as_str().to_owned(),
        navigation.to(Route::Project(id, ProjectPage::Overview(OverviewTab::default()))),
      ),
      Crumb::to(
        ctx.t("pull_requests.title"),
        navigation.to(Route::Project(id, ProjectPage::PullRequests(PullRequestTab::default()))),
      ),
      Crumb::here(ctx.t_args("pull_requests.row.number", [("number", number.to_string())])).mono(),
    ],
  )
  .into()
}

/// The title, cut with an ellipsis where it would run under its number, and the number, Refresh and Open in
/// GitHub whole.
fn title_row(ctx: &mut Ctx, pull_request: &PullRequest, actions: &Actions) -> Row {
  let refresh = ActionButton::new(
    "pull-request-refresh",
    ButtonKind::Ghost,
    ctx.t("pull_requests.refresh"),
  )
  .icon(Icon::RefreshCw);
  let refresh = match actions.refresh.clone() {
    Some(run) => refresh.on_click(move || run()),
    None => refresh.disabled(),
  };
  let github = ActionButton::new(
    "pull-request-github",
    ButtonKind::Secondary,
    ctx.t("pull_requests.row.openInGitHub"),
  )
  .icon(Icon::ExternalLink);
  let github = match actions.open.clone() {
    Some(run) => github.on_click(move || run()),
    None => github.disabled(),
  };
  let title = Truncated::new(
    "pull-request-title",
    Arc::<str>::from(pull_request.title()),
    TypographyStyle::Heading,
  )
  .render(ctx);
  Row::new()
    .id("pull-request-title-row")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
        .child(title),
    )
    .child(
      Text::new(&ctx.t_args(
        "pull_requests.row.number",
        [("number", pull_request.number().to_string())],
      ))
      .id("pull-request-number")
      .variant(PullRequestsText::TitleNumber.role())
      .flex_shrink(0.0)
      .nowrap(),
    )
    .child(Row::new().flex(1.0))
    .child(refresh)
    .child(github)
}

/// The state, the review, the branches, the task and the checks, and at the end the head and how the latest
/// read went.
fn organization(
  ctx: &mut Ctx,
  project: &ShownProject,
  view: &PullRequestView,
  pull_request: &PullRequest,
  reading: bool,
) -> Row {
  let separator = || {
    Row::new()
      .width(Dimension::Px(1.0))
      .height(Dimension::Px(SEPARATOR_HEIGHT))
      .background(PaletteColor::Border)
  };
  let branches = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::GitBranch, IconSize::Base, PaletteColor::TextMuted))
    .child(branch(pull_request.head().as_str()))
    .child(icon(Icon::ArrowRight, IconSize::Sm, PaletteColor::TextMuted))
    .child(branch(pull_request.base().as_str()));
  let task = view.task.as_ref().map(|task| {
    let navigation = Navigation::of(ctx);
    let to = Route::Project(
      project.open.project.id(),
      ProjectPage::Task(task.key.clone(), TaskView::of(TaskTab::Summary)),
    );
    linked_task(ctx, &task.key.to_string(), navigation.to(to))
  });
  let mut row = Row::new()
    .id("pull-request-organization")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ORGANIZATION_HEIGHT))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(labels::state(ctx, pull_request))
    .child(labels::review(ctx, pull_request, TypographyStyle::Label))
    .child(separator())
    .child(branches);
  if let Some(task) = task {
    row = row.child(separator()).child(task);
  }
  if let Some(checks) = labels::checks(ctx, pull_request) {
    row = row.child(separator()).child(checks);
  }
  row
    .child(Row::new().flex(1.0))
    .child(context(ctx, view, pull_request, reading))
}

fn branch(name: &str) -> Text {
  Text::new(name)
    .variant(ExtraTypography::PropertyMono.role())
    .color(PaletteColor::TextSecondary)
    .nowrap()
}

/// *Task* and the task's key in its link (`Linked task link` `EVzFC`), which opens the task.
fn linked_task(ctx: &Ctx, key: &str, open: Open) -> Row {
  let link = opening(open)
    .id("pull-request-task")
    .height(Dimension::Px(ControlHeight::Sm.value()))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Snug.role())
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .describe("a11y_name", key.to_owned())
    .child(icon(Icon::ListChecks, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(key)
        .variant(ExtraTypography::MonoKey.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(icon(Icon::ArrowUpRight, IconSize::Xs, PaletteColor::TextMuted));
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("pull_requests.detail.task"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(link)
}

/// `Head 7c1e9f2 · Synced just now`, with the status's icon when the latest read failed.
fn context(ctx: &Ctx, view: &PullRequestView, pull_request: &PullRequest, reading: bool) -> Row {
  let status = view.status();
  let failed = view.latest_failure().is_some();
  let shown = if reading { Shown::Syncing } else { Shown::After(status) };
  let (glyph, mark, color, words) = parts(ctx, &shown, Timestamp::now());
  let text = ctx.t_args(
    "pull_requests.detail.context",
    [
      (
        "revision",
        display::short_revision(pull_request.head_sha().as_str()).to_owned(),
      ),
      ("status", words.to_string()),
    ],
  );
  Row::new()
    .id("pull-request-context")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .with_children((failed || reading).then(|| icon(glyph, IconSize::Sm, mark)))
    .child(
      Text::new(&text)
        .variant(ExtraTypography::Note.role())
        .color(if failed { color } else { PaletteColor::TextMuted })
        .nowrap(),
    )
}

/// Files changed with its count once known, Agent feedback with its open threads once read, then Agent checks and
/// Overview, locked, and why at the row's end.
fn tabs(ctx: &mut Ctx, (project, number): (&ShownProject, u64), (count, shown): (Option<usize>, &Tabs)) -> Element {
  let label = |ctx: &Ctx, key: &str, count: Option<usize>| {
    let label = ctx.t(key);
    match count {
      Some(count) => counted(ctx, &label, count),
      None => label,
    }
  };
  let files = label(ctx, "pull_requests.detail.tabs.files", count);
  let feedback = label(ctx, "pull_requests.detail.tabs.feedback", shown.feedback_count);
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let page = NonZeroU64::new(number).expect("a pull request's page has its number");
  let planned = ctx.t("pull_requests.detail.tabs.planned");
  let tabs = [
    PageTab::to(
      "pull-request-tab-files",
      files,
      !shown.feedback_shown,
      navigation.to(Route::Project(id, ProjectPage::PullRequest(page, None))),
    ),
    PageTab::to(
      "pull-request-tab-feedback",
      feedback,
      shown.feedback_shown,
      navigation.to(Route::Project(id, ProjectPage::PullRequestFeedback(page, None))),
    ),
    PageTab::unavailable(
      "pull-request-tab-checks",
      ctx.t("pull_requests.detail.tabs.checks"),
      planned.clone(),
    ),
    PageTab::unavailable(
      "pull-request-tab-overview",
      ctx.t("pull_requests.detail.tabs.overview"),
      planned.clone(),
    ),
  ];
  page_tabs(ctx, "pull-request-tabs", tabs)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(TABS_HEIGHT))
        .justify(Justify::End)
        .align_items(Alignment::Center)
        .pointer_events_none()
        .child(Row::new().child(reason_line("pull-request-tabs-reason", &planned).width(Dimension::Auto))),
    )
    .into()
}
