//! The toolbar under a pull request's header on Files changed (`Changes toolbar` `C4AKw`): which revisions the diff
//! shows (`Range` `jHagN`): the whole pull request, or what changed since a fix's request (*Since your request*,
//! `x6EGy` in `OmUih`) with what that leaves out; the head a new thread is being written at, kept while the pull
//! request moved on (*Kept while you write*, `NTYD9`); and the totals. While the diff shown is not the pull
//! request's head, a notice says so with *Show …* (`Head notice` `K4J7v`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, SelectOption, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::FeedbackId, pull_request::PullRequest, revision::Revision};
use orchester_workspace::{
  display::short_revision,
  pull_requests::{detail::ReadDiff, page::PullRequestView},
};

use super::{super::labels::grouped, feedback::FeedbackState};
use crate::{
  theme::{ExtraTypography, IconSize, PullRequestsSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    field::SelectField,
    icon::{Icon, icon},
  },
};

/// The toolbar's height (`C4AKw`) and the range's width (`jHagN`; `x6EGy` with a request's range).
const TOOLBAR_HEIGHT: f32 = 44.0;
const RANGE_WIDTH: f32 = 248.0;
const SINCE_WIDTH: f32 = 340.0;

/// The diff shown instead of the pull request's latest.
#[derive(Clone)]
pub struct Instead {
  pub read: Arc<ReadDiff>,
  pub why: InsteadWhy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsteadWhy {
  /// The head a thread was written at (*Show at*), or a new thread is being written at (`writing`).
  Earlier { writing: bool },
  /// What changed since a fix's request.
  Since,
}

/// A fix whose commit the range can show what changed since its request.
#[derive(Clone)]
pub struct Fixed {
  pub thread: FeedbackId,
  pub from: Revision,
  pub to: Revision,
}

/// What the toolbar is drawn with.
pub struct Range<'a> {
  pub view: &'a PullRequestView,
  pub pull_request: &'a PullRequest,
  pub instead: Option<&'a Instead>,
  pub fixed: &'a [Fixed],
  pub state: Option<&'a FeedbackState>,
}

pub fn toolbar(ctx: &mut Ctx, range: &Range<'_>) -> Row {
  let head = short_revision(range.pull_request.head_sha().as_str()).to_owned();
  let full = match &range.view.diff {
    orchester_workspace::pull_requests::page::PageDiff::Read(read) => ctx.t_args(
      "pull_requests.detail.range",
      [("base", short_revision(read.base.as_str()).to_owned()), ("head", head)],
    ),
    _ => ctx.t_args("pull_requests.detail.rangeHead", [("head", head)]),
  };
  let since = range.state.map(|state| state.since.clone());
  let mut options = vec![SelectOption::new(None, full.clone())];
  options.extend(range.fixed.iter().map(|fixed| {
    SelectOption::new(
      Some(fixed.thread),
      ctx.t_args(
        "pull_requests.feedback.range.since",
        [
          ("from", short_revision(fixed.from.as_str()).to_owned()),
          ("to", short_revision(fixed.to.as_str()).to_owned()),
        ],
      ),
    )
  }));
  let width = if since.as_ref().is_some_and(|since| since.get().is_some()) {
    SINCE_WIDTH
  } else {
    RANGE_WIDTH
  };
  let select = match since {
    Some(since) => SelectField::new(
      "pull-request-range",
      ctx.t("pull_requests.feedback.range.choose"),
      since,
      options,
    )
    .mono()
    .width(Dimension::Px(width))
    .render(ctx),
    None => Text::new(&full)
      .id("pull-request-range")
      .variant(ExtraTypography::MonoKey.role())
      .into(),
  };
  Row::new()
    .id("pull-request-toolbar")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOOLBAR_HEIGHT))
    .padding(Padding::horizontal(PullRequestsSpacing::Detail.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(select)
    .with_children(note(ctx, range))
    .child(Row::new().flex(1.0))
    .child(
      Text::new(&total(ctx, range))
        .id("pull-request-total")
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
}

/// What the range leaves out: kept while a new thread is written, or the whole pull request's size.
fn note(ctx: &Ctx, range: &Range<'_>) -> Option<Text> {
  let text = match range.instead?.why {
    InsteadWhy::Earlier { writing: true } => ctx.t("pull_requests.feedback.head.kept"),
    InsteadWhy::Earlier { writing: false } => return None,
    InsteadWhy::Since => ctx.t_args(
      "pull_requests.feedback.range.sinceNote",
      [("files", grouped(ctx, range.pull_request.changes().files))],
    ),
  };
  Some(
    Text::new(&text)
      .id("pull-request-range-note")
      .variant(ExtraTypography::Note.role())
      .nowrap(),
  )
}

/// `6 files · +133 / −15`: the pull request's totals, or what the diff shown changes when it is another range.
fn total(ctx: &Ctx, range: &Range<'_>) -> Arc<str> {
  let (files, additions, deletions) = match range.instead.filter(|instead| instead.why == InsteadWhy::Since) {
    Some(instead) => (
      u32::try_from(instead.read.diff.files().len()).unwrap_or(u32::MAX),
      instead.read.diff.additions(),
      instead.read.diff.deletions(),
    ),
    None => {
      let changes = range.pull_request.changes();
      (changes.files, changes.additions, changes.deletions)
    }
  };
  let key = if files == 1 {
    "pull_requests.detail.total.one"
  } else {
    "pull_requests.detail.total.other"
  };
  ctx.t_args(
    key,
    [
      ("files", grouped(ctx, files)),
      ("additions", grouped(ctx, additions)),
      ("deletions", grouped(ctx, deletions)),
    ],
  )
}

/// The diff shown is at an earlier head than the pull request's: which, and *Show …* the head (`K4J7v`).
pub fn head_notice(ctx: &Ctx, (instead, head, state): (&Instead, &Revision, &FeedbackState)) -> Option<Element> {
  let InsteadWhy::Earlier { writing } = instead.why else {
    return None;
  };
  let (from, to) = (
    short_revision(instead.read.head.as_str()).to_owned(),
    short_revision(head.as_str()).to_owned(),
  );
  let key = if writing {
    "pull_requests.feedback.head.notice"
  } else {
    "pull_requests.feedback.head.noticeAt"
  };
  let (pinned, follow) = (state.pinned.clone(), state.follow.clone());
  Some(
    Row::new()
      .id("pull-request-head-notice")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Sm))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceRaised)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(icon(
        Icon::GitCommitHorizontal,
        IconSize::Base,
        PaletteColor::TextSecondary,
      ))
      .child(
        Text::new(&ctx.t_args(key, [("from", from), ("to", to.clone())]))
          .variant(TypographyStyle::Description)
          .nowrap(),
      )
      .child(Row::new().flex(1.0))
      .child(
        ActionButton::new(
          "pull-request-show-head",
          ButtonKind::Ghost,
          ctx.t_args("pull_requests.feedback.head.show", [("to", to)]),
        )
        .icon(Icon::RefreshCw)
        .small()
        .on_click(move || {
          pinned.set(None);
          follow.set(true);
        }),
      )
      .into(),
  )
}
