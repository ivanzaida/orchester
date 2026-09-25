//! Pull request list row (`QHBNZ`) and the column headings above the rows (`Qcgjy`). Rows are
//! pointer-only and show GitHub's state of the pull request; agent work and review in Orchester
//! come with runs, so every row reads "No linked task". The review screen a row opens in the
//! design (`Rrmhf`) does not exist yet: the row is not clickable and shows a lock, and Open in
//! GitHub opens the pull request in the browser.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  pull_request::{CheckRollup, PullRequest, PullRequestState, ReviewDecision},
  time::Timestamp,
};

use crate::{
  display,
  messages::Message,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    message_text,
    status_label::status_label,
  },
};

/// Row height and the state and checks columns (`QHBNZ`, `ruZbF`, `ZWzfm`, `tPwvi`).
const ROW_HEIGHT: f32 = 100.0;
const STATE_WIDTH: f32 = 180.0;
const CHECKS_WIDTH: f32 = 150.0;
/// Open in GitHub, fixed so the column headings line up with the rows, and the lock after it.
const OPEN_WIDTH: f32 = 128.0;
const LOCK_WIDTH: f32 = 12.0;
/// Characters of a revision shown in a row ("8f3a2c1").
const SHORT_REVISION: usize = 7;
/// Height of the Open in GitHub button, as the Copy button of a command row (`eFsWL`).
const OPEN_HEIGHT: f32 = 26.0;

type Handler = Arc<dyn Fn() + Send + Sync>;

pub fn column_headings(ctx: &Ctx) -> Element {
  let heading = |key: &str| {
    Text::new(&ctx.t(key))
      .variant(ExtraTypography::Footnote.role())
      .nowrap()
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(heading("pullRequests.columns.pullRequest").flex(1.0))
    .child(heading("pullRequests.columns.state").width(Dimension::Px(STATE_WIDTH)))
    .child(heading("pullRequests.columns.checks").width(Dimension::Px(CHECKS_WIDTH)))
    .child(Row::new().width(Dimension::Px(OPEN_WIDTH)))
    .child(Row::new().width(Dimension::Px(LOCK_WIDTH)))
    .into()
}

/// A row's pull request and what Open in GitHub does with it.
#[derive(Clone)]
pub struct RowProps {
  pub pull_request: PullRequest,
  pub open: Handler,
}

impl PartialEq for RowProps {
  fn eq(&self, other: &Self) -> bool {
    self.pull_request == other.pull_request
  }
}

impl lurq::app::component::DevtoolsInspectable for RowProps {}

/// One row of the list, mounted by the virtualized list.
pub struct PullRequestRow;

impl Component for PullRequestRow {
  type Props = RowProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let RowProps { pull_request, open } = ctx.props::<RowProps>().clone();
    Row::new()
      .id("pull-request-row")
      .describe("number", pull_request.number().to_string())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ROW_HEIGHT))
      .padding_vertical(SpacingSize::Md)
      .padding_horizontal(ExtraSpacing::Stack.role())
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .cursor(CursorIcon::NotAllowed)
      .child(icon(Icon::GitPullRequest, IconSize::Row, PaletteColor::TextPrimary))
      .child(identity(ctx, &pull_request, Timestamp::now()))
      .child(review(ctx, &pull_request))
      .child(checks(ctx, pull_request.checks()))
      .child(
        ActionButton::new(ctx.t("pullRequests.row.openInGitHub"))
          .icon(Icon::ExternalLink)
          .compact()
          .id("pull-request-open")
          .size(Dimension::Px(OPEN_WIDTH), Dimension::Px(OPEN_HEIGHT))
          .on_click(move || open())
          .render(ctx),
      )
      // The review screen the row opens in the design does not exist yet: a lock where the
      // design has the open chevron (`laxWE`), as on task rows.
      .child(
        Row::new()
          .width(Dimension::Px(LOCK_WIDTH))
          .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted)),
      )
  }
}

/// Title, number and branches, and the size of the change (`ljxQE`).
fn identity(ctx: &Ctx, pull_request: &PullRequest, now: Timestamp) -> Element {
  let number = pull_request.number().to_string();
  let meta = ctx.t_args(
    "pullRequests.row.meta",
    [
      ("number", number.as_str()),
      ("head", pull_request.head().as_str()),
      ("base", pull_request.base().as_str()),
    ],
  );
  let changes = pull_request.changes();
  let key = if changes.files == 1 {
    "pullRequests.row.changesOne"
  } else {
    "pullRequests.row.changesOther"
  };
  let summary = Message::new(key)
    .with("files", changes.files.to_string())
    .with("additions", changes.additions.to_string())
    .with("deletions", changes.deletions.to_string())
    .with(
      "updated",
      message_text(ctx, &display::elapsed(now, pull_request.updated_at())).to_string(),
    );
  let line = |text: &str, typography: TypographyStyle| {
    Text::new(text)
      .variant(typography)
      .nowrap()
      .text_overflow(TextOverflow::Elipsis)
  };
  Column::new()
    .flex(1.0)
    .spacing(SpacingSize::Sm)
    .child(line(pull_request.title(), ExtraTypography::RowTitle.role()))
    .child(line(&meta, TypographyStyle::Caption))
    .child(line(&message_text(ctx, &summary), TypographyStyle::Mono).color(PaletteColor::TextSecondary))
    .into()
}

/// GitHub's state of the pull request and what reviews decided (`ruZbF`).
fn review(ctx: &Ctx, pull_request: &PullRequest) -> Element {
  let decision = match pull_request.review_decision() {
    None => "pullRequests.review.none",
    Some(ReviewDecision::ReviewRequired) => "pullRequests.review.reviewRequired",
    Some(ReviewDecision::Approved) => "pullRequests.review.approved",
    Some(ReviewDecision::ChangesRequested) => "pullRequests.review.changesRequested",
  };
  let state = pull_request.state();
  Column::new()
    .width(Dimension::Px(STATE_WIDTH))
    .spacing(SpacingSize::Sm)
    .child(Row::new().child(status_label(state_icon(state), &ctx.t(state_key(state)))))
    .child(Text::new(&ctx.t(decision)).variant(TypographyStyle::Caption))
    .into()
}

/// The checks of the head revision (`ZWzfm`).
fn checks(ctx: &Ctx, checks: &CheckRollup) -> Element {
  let count = |value: u32| value.to_string();
  let total = count(checks.total());
  let summary = if checks.total() == 0 {
    ctx.t("pullRequests.checks.none")
  } else if checks.failed > 0 {
    ctx.t_args(
      "pullRequests.checks.failed",
      [("failed", count(checks.failed).as_str()), ("total", total.as_str())],
    )
  } else if checks.pending > 0 {
    ctx.t_args(
      "pullRequests.checks.pending",
      [("pending", count(checks.pending).as_str()), ("total", total.as_str())],
    )
  } else if checks.skipped > 0 {
    ctx.t_args(
      "pullRequests.checks.passedSkipped",
      [
        ("passed", count(checks.passed).as_str()),
        ("skipped", count(checks.skipped).as_str()),
      ],
    )
  } else if checks.passed == 1 {
    ctx.t("pullRequests.checks.passedOne")
  } else {
    ctx.t_args("pullRequests.checks.passedOther", [("count", total.as_str())])
  };
  let revision = &checks.on.as_str()[..SHORT_REVISION];
  Column::new()
    .width(Dimension::Px(CHECKS_WIDTH))
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&summary)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary),
    )
    .child(
      Text::new(&ctx.t_args("pullRequests.checks.on", [("revision", revision)]))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextSecondary),
    )
    .into()
}

const fn state_icon(state: PullRequestState) -> Icon {
  match state {
    PullRequestState::Open => Icon::GitPullRequest,
    PullRequestState::Draft => Icon::GitPullRequestDraft,
    PullRequestState::Merged => Icon::GitMerge,
    PullRequestState::Closed => Icon::GitPullRequestClosed,
  }
}

const fn state_key(state: PullRequestState) -> &'static str {
  match state {
    PullRequestState::Open => "pullRequests.state.open",
    PullRequestState::Draft => "pullRequests.state.draft",
    PullRequestState::Merged => "pullRequests.state.merged",
    PullRequestState::Closed => "pullRequests.state.closed",
  }
}
