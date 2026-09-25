//! The pull request heading (`y4WZMq`): its title with GitHub's state (`h4QMei`), and the mono
//! context line under it (`ADELW`): number, branches, the head revision the diff shows, and how
//! current it is ("Synced just now"). Its own component with its own interval, so the relative
//! time stays current without re-rendering the diff.

use std::time::Duration;

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{pull_request::PullRequest, revision::Revision, time::Timestamp};

use super::super::{
  row::{state_icon, state_key},
  view::SyncLine,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::status_label::status_label,
};

/// Relative times are shown in whole minutes; a half-minute tick keeps them at most that stale.
const TICK: Duration = Duration::from_secs(30);
/// Characters of a revision the context line shows ("8f3a2c1").
pub const SHORT_REVISION: usize = 7;

#[derive(Clone, PartialEq, Eq)]
pub struct HeadingProps {
  pub number: u64,
  pub pull_request: Option<PullRequest>,
  /// The head revision of the diff on screen, else of the pull request.
  pub head: Option<Revision>,
  pub line: SyncLine,
}

impl lurq::app::component::DevtoolsInspectable for HeadingProps {}

pub struct PullRequestHeading {
  tick: Signal<u64>,
}

impl Component for PullRequestHeading {
  type Props = HeadingProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    ctx
      .create_interval(TICK, move || next.update(|tick| *tick += 1))
      .start();
    Self { tick }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Read so every tick re-renders the relative time.
    self.tick.get();
    let props = ctx.props::<HeadingProps>().clone();
    let number = props.number.to_string();
    let title = match &props.pull_request {
      Some(pull_request) => pull_request.title().into(),
      None => ctx.t_args("pullRequests.detail.untitled", [("number", number.as_str())]),
    };
    let mut title_row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Group.role())
      .align_items(Alignment::Center)
      .child(
        Text::new(&title)
          .variant(ExtraTypography::PullRequestTitle.role())
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      );
    if let Some(pull_request) = &props.pull_request {
      let state = pull_request.state();
      title_row = title_row.child(status_label(state_icon(state), &ctx.t(state_key(state))));
    }
    Column::new()
      .id("pull-request-heading")
      .width(Dimension::Pct(100.0))
      .padding_top(ExtraSpacing::Step.role())
      .padding_bottom(ExtraSpacing::Divider.role())
      .padding_horizontal(SpacingSize::Lg)
      .spacing(ExtraSpacing::Stack.role())
      .child(title_row)
      .child(
        Text::new(&context(ctx, &props, &number))
          .id("pull-request-context")
          .variant(ExtraTypography::PullRequestContext.role())
          .nowrap(),
      )
  }
}

/// "PR #42 · orc/github-auth → main · Head 8f3a2c1 · Synced just now".
fn context(ctx: &Ctx, props: &HeadingProps, number: &str) -> String {
  let sync = props.line.text(ctx, Timestamp::now());
  let Some(pull_request) = &props.pull_request else {
    return ctx
      .t_args(
        "pullRequests.detail.contextUnknown",
        [("number", number), ("sync", &*sync)],
      )
      .to_string();
  };
  let head = props.head.as_ref().unwrap_or_else(|| pull_request.head_sha());
  ctx
    .t_args(
      "pullRequests.detail.context",
      [
        ("number", number),
        ("head", pull_request.head().as_str()),
        ("base", pull_request.base().as_str()),
        ("revision", &head.as_str()[..SHORT_REVISION]),
        ("sync", &*sync),
      ],
    )
    .to_string()
}
