//! One pull request (`Pull request list row` `aFRGq`): its title, number, the task its branch is of (the task's
//! key opens it), its branches, its changes and when it was updated; its state over what reviews decided; its
//! GitHub checks over the revision they ran on; Open in GitHub; and the chevron. A press on the row opens the
//! pull request in the app.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ProjectId,
  pull_request::{PullRequest, PullRequestState, ReviewDecision},
  time::Timestamp,
};
use orchester_workspace::{display, pull_requests::list::ListedPullRequest};

use super::work::OpenAction;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    pressable::pressable,
    table::{ColumnWidth, TableRow},
    tooltip::Truncated,
  },
};

/// The columns' widths (`Pull request list header` `ODk8t`): the pull request the room left, State & review
/// 150, GitHub checks 170, Open in GitHub 129.
pub(super) const PULL_REQUEST: ColumnWidth = ColumnWidth::Fill;
pub(super) const STATE: ColumnWidth = ColumnWidth::Fixed(150.0);
pub(super) const CHECKS: ColumnWidth = ColumnWidth::Fixed(170.0);
pub(super) const OPEN: ColumnWidth = ColumnWidth::Fixed(129.0);
/// A row's height: its three lines inside 12 over and under them.
pub(super) const ROW_HEIGHT: f32 = 78.0;
const MINUTE_MILLIS: i64 = 60_000;

#[derive(Clone)]
pub(super) struct RowProps {
  pub project: ProjectId,
  pub listed: ListedPullRequest,
  pub now: Timestamp,
  pub open: OpenAction,
}

/// Rows are compared by what they show; Open in GitHub goes to the same place.
impl PartialEq for RowProps {
  fn eq(&self, other: &Self) -> bool {
    (self.project, &self.listed, self.now) == (other.project, &other.listed, other.now)
  }
}

impl DevtoolsInspectable for RowProps {}

pub(super) struct PullRequestRow;

impl Component for PullRequestRow {
  type Props = RowProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let pull_request = &props.listed.pull_request;
    let number = pull_request.number();
    let navigation = Navigation::of(ctx);
    let opened = Route::Project(props.project, ProjectPage::PullRequest(number));
    let url: Arc<str> = Arc::from(pull_request.url());
    let open = props.open.clone();
    let github = ActionButton::new(
      format!("pull-request-github-{number}"),
      ButtonKind::Ghost,
      ctx.t("pull_requests.row.openInGitHub"),
    )
    .icon(Icon::ExternalLink)
    .on_click(move || open.run((number, url.clone())));
    TableRow::new(format!("pull-request-row-{number}"))
      .height(ROW_HEIGHT)
      .cell(PULL_REQUEST, identity(ctx, &props))
      .cell(STATE, state_cell(ctx, pull_request))
      .cell(CHECKS, checks_cell(ctx, pull_request))
      .cell(OPEN, github)
      .on_open(move || navigation.go(opened.clone()))
  }
}

/// The title, the number with the task and the branches, and the changes with when it was updated.
fn identity(ctx: &mut Ctx, props: &RowProps) -> Column {
  let pull_request = &props.listed.pull_request;
  let number = pull_request.number();
  let changes = pull_request.changes();
  let updated = updated(ctx, props.now, pull_request.updated_at());
  let files = if changes.files == 1 {
    "pull_requests.row.changes.one"
  } else {
    "pull_requests.row.changes.other"
  };
  let changes = ctx.t_args(
    files,
    [
      ("files", changes.files.to_string()),
      ("additions", changes.additions.to_string()),
      ("deletions", changes.deletions.to_string()),
      ("updated", updated.to_string()),
    ],
  );
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xs)
    .child(
      Truncated::new(
        format!("pull-request-title-{number}"),
        Arc::from(pull_request.title()),
        ExtraTypography::Strong.role(),
      )
      .render(ctx),
    )
    .child(meta(ctx, props))
    .child(
      Text::new(&changes)
        .id(format!("pull-request-changes-{number}"))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap(),
    )
}

/// `#142 · ≡ KON-12 · kon-12-webhook-retry → main`; *No linked task* when no run made the branch.
fn meta(ctx: &mut Ctx, props: &RowProps) -> Row {
  let pull_request = &props.listed.pull_request;
  let number = pull_request.number();
  let navigation = Navigation::of(ctx);
  let dot = ctx.t("pull_requests.row.separator");
  let separator = || Text::new(&dot).variant(ExtraTypography::Note.role());
  let task: Element = match &props.listed.task {
    Some(task) => {
      let to = Route::Project(
        props.project,
        ProjectPage::Task(task.key.clone(), TaskView::of(TaskTab::Summary)),
      );
      pressable(move || navigation.go(to.clone()))
        .id(format!("pull-request-task-{number}"))
        .spacing(SpacingSize::Xs)
        .describe("a11y_name", task.key.to_string())
        .child(icon(Icon::ListChecks, IconSize::Sm, PaletteColor::TextSecondary))
        .child(
          Text::new(&task.key.to_string())
            .variant(ExtraTypography::MonoKey.role())
            .color(PaletteColor::TextSecondary)
            .nowrap(),
        )
        .into()
    }
    None => Text::new(&ctx.t("pull_requests.row.noTask"))
      .id(format!("pull-request-task-{number}"))
      .variant(ExtraTypography::Note.role())
      .nowrap()
      .into(),
  };
  let branches = ctx.t_args(
    "pull_requests.row.branches",
    [
      ("head", pull_request.head().as_str()),
      ("base", pull_request.base().as_str()),
    ],
  );
  Row::new()
    .id(format!("pull-request-meta-{number}"))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t_args("pull_requests.row.number", [("number", number.to_string())]))
        .variant(ExtraTypography::MonoKey.role())
        .nowrap(),
    )
    .child(separator())
    .child(task)
    .child(separator())
    .child(Text::new(&branches).variant(ExtraTypography::MonoKey.role()).nowrap())
}

/// *Updated just now* within a minute, else how long ago.
fn updated(ctx: &Ctx, now: Timestamp, at: Timestamp) -> Arc<str> {
  if now.unix_millis() - at.unix_millis() < MINUTE_MILLIS {
    return ctx.t("pull_requests.row.justNow");
  }
  let ago = message_text(ctx, &display::waited(now, at));
  ctx.t_args("pull_requests.row.ago", [("ago", ago)])
}

/// The state over what reviews decided.
fn state_cell(ctx: &Ctx, pull_request: &PullRequest) -> Column {
  use PaletteColor::{Success, TextMuted, TextPrimary, TextSecondary, Warning};
  let number = pull_request.number();
  let (glyph, mark, label, key) = match pull_request.state() {
    PullRequestState::Open => (
      Icon::GitPullRequest,
      TextSecondary,
      TextPrimary,
      "pull_requests.state.open",
    ),
    PullRequestState::Draft => (
      Icon::GitPullRequestDraft,
      TextMuted,
      TextSecondary,
      "pull_requests.state.draft",
    ),
    PullRequestState::Merged => (Icon::GitMerge, Success, TextPrimary, "pull_requests.state.merged"),
    PullRequestState::Closed => (
      Icon::GitPullRequestClosed,
      TextMuted,
      TextSecondary,
      "pull_requests.state.closed",
    ),
  };
  let review = match pull_request.review_decision() {
    Some(ReviewDecision::ReviewRequired) => Some((Icon::Eye, Warning, "pull_requests.review.required")),
    Some(ReviewDecision::Approved) => Some((Icon::CircleCheck, Success, "pull_requests.review.approved")),
    Some(ReviewDecision::ChangesRequested) => {
      Some((Icon::FileDiff, TextSecondary, "pull_requests.review.changesRequested"))
    }
    None => None,
  };
  let review = match review {
    Some((glyph, color, key)) => Row::new()
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Sm, color.clone()))
      .child(
        Text::new(&ctx.t(key))
          .variant(ExtraTypography::Note.role())
          .color(color)
          .nowrap(),
      ),
    None => Row::new().child(
      Text::new(&ctx.t("pull_requests.review.none"))
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    ),
  };
  Column::new()
    .id(format!("pull-request-state-{number}"))
    .spacing(ExtraSpacing::Pair.role())
    .child(
      Row::new()
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Base, mark))
        .child(
          Text::new(&ctx.t(key))
            .variant(TypographyStyle::Label)
            .color(label)
            .nowrap(),
        ),
    )
    .child(review)
}

/// What GitHub's checks of the head came to, over the revision they ran on; *No checks* when none ran.
fn checks_cell(ctx: &Ctx, pull_request: &PullRequest) -> Column {
  let number = pull_request.number();
  let checks = pull_request.checks();
  let cell = Column::new()
    .id(format!("pull-request-checks-{number}"))
    .spacing(ExtraSpacing::Pair.role());
  if checks.total() == 0 {
    return cell.child(
      Text::new(&ctx.t("pull_requests.checks.none"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    );
  }
  let counts = [
    (
      checks.passed,
      Icon::CircleCheck,
      PaletteColor::Success,
      PaletteColor::TextSecondary,
      "pull_requests.checks.passed",
    ),
    (
      checks.pending,
      Icon::LoaderCircle,
      PaletteColor::TextSecondary,
      PaletteColor::TextSecondary,
      "pull_requests.checks.running",
    ),
    (
      checks.failed,
      Icon::CircleX,
      PaletteColor::Danger,
      PaletteColor::Danger,
      "pull_requests.checks.failed",
    ),
    (
      checks.skipped,
      Icon::CircleMinus,
      PaletteColor::TextMuted,
      PaletteColor::TextMuted,
      "pull_requests.checks.skipped",
    ),
  ];
  let counts = counts
    .into_iter()
    .filter(|(count, ..)| *count > 0)
    .map(|(count, glyph, mark, color, key)| {
      Row::new()
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Sm, mark))
        .child(
          Text::new(&ctx.t_args(key, [("count", count.to_string())]))
            .variant(TypographyStyle::Description)
            .color(color)
            .nowrap(),
        )
    });
  let revision = ctx.t_args(
    "pull_requests.checks.on",
    [("revision", display::short_revision(checks.on.as_str()))],
  );
  cell
    .child(
      Row::new()
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Center)
        .with_children(counts),
    )
    .child(Text::new(&revision).variant(ExtraTypography::MonoNote.role()).nowrap())
}
